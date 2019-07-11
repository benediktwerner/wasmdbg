extern crate parity_wasm;

use parity_wasm::elements::{Module, Instruction, ValueType, FuncBody, Type::Function};


#[derive(Clone)]
pub enum Trap {
    ReachedUnreachable,
    UnknownInstruction(Instruction),
    PopFromEmptyStack,
    ExecutionFinished,
}

type VMResult<T> = Result<T, Trap>;

enum Value {
    I32(u32),
    I64(u64),
    F32(f32),
    F64(f64),
    V128(u128),
}

impl Value {
    fn default(value_type: ValueType) -> Self {
        match value_type {
            ValueType::I32 => Value::I32(0),
            ValueType::I64 => Value::I64(0),
            ValueType::F32 => Value::F32(0.0),
            ValueType::F64 => Value::F64(0.0),
            ValueType::V128 => Value::V128(0),
        }
    }
}

#[derive(Default)]
struct CodePosition {
    func_index: usize,
    instr_index: usize,
}

struct Label {
    target_instr_index: Option<usize>,
}

impl Label {
    fn new(target_instr_index: Option<usize>) -> Self {
        Label {target_instr_index}
    }
}

struct FunctionFrame {
    ret_addr: CodePosition,
    locals: Vec<Value>,
}

struct Memory {
    data: Vec<u8>,
}

pub struct VM {
    module: Box<Module>,
    memory: Memory,
    ip: CodePosition,
    globals: Vec<Value>,
    value_stack: Vec<Value>,
    label_stack: Vec<Label>,
    function_stack: Vec<FunctionFrame>,
    trap: Option<Trap>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory { data: Vec::new() }
    }
}

impl VM {
    pub fn new(module: Box<Module>) -> VM {
        let mut globals = match module.global_section() {
            Some(global_section) => {
                let mut globals = Vec::with_capacity(global_section.entries().len());
                for global in global_section.entries() {
                    globals.push(Value::default(global.global_type().content_type()));
                }
                globals
            },
            None => Vec::new(),
        };
        VM {
            memory: Memory::new(),
            module,
            ip: CodePosition::default(),
            globals,
            value_stack: Vec::new(),
            label_stack: Vec::new(),
            function_stack: Vec::new(),
            trap: None,
        }
    }

    fn trap(&mut self, trap: Trap) {
        if self.trap.is_none() {
            self.trap = Some(trap);
        }
    }

    fn push(&mut self, val: Value) {
        self.value_stack.push(val);
    }

    fn pop(&mut self) -> VMResult<Value> {
        self.value_stack.pop().ok_or(Trap::PopFromEmptyStack)
    }

    fn locals(&self) -> &[Value] {
        &self.function_stack.last().unwrap().locals
    }

    fn curr_func(&self) -> &FuncBody {
        &self.module.code_section().unwrap().bodies()[self.ip.func_index]
    }

    fn curr_code(&self) -> &[Instruction] {
        self.curr_func().code().elements()
    }

    fn branch(&mut self, index: u32) {
        self.label_stack.truncate(self.label_stack.len() - index as usize);
        match self.label_stack.pop().unwrap().target_instr_index {
            Some(target) => self.ip.instr_index = target,
            None => {
                loop {
                    match self.curr_code()[self.ip.instr_index] {
                        Instruction::Block(_) => index += 1,
                        Instruction::Loop(_) => index += 1,
                        Instruction::End => index -= 1,
                    }

                    if index == 0 {
                        break;
                    }
                }
            },
        }
    }

    pub fn execute_step(&mut self) -> VMResult<()> {
        let func = self.curr_func();
        let instr = func.code().elements()[self.ip.instr_index].clone();
        self.ip.instr_index += 1;

        match instr {
            Instruction::Unreachable => self.trap(Trap::ReachedUnreachable),
            Instruction::Nop => (),
            Instruction::Block(_) => self.label_stack.push(Label::new(None)),
            Instruction::Loop(_) => self.label_stack.push(Label::new(Some(self.ip.instr_index))),
            Instruction::If(_) => (),
            Instruction::Else => (),
            Instruction::End => {
                if self.label_stack.pop().is_none() {
                    let frame = self.function_stack.pop().unwrap();
                    self.ip = frame.ret_addr;
                }
            },
            Instruction::Br(index) => self.branch(index),
            Instruction::BrIf(index) => {
                if let Value::I32(val) = self.pop()? {
                    if val != 0 {
                        self.branch(index);
                    }
                }
                else {
                    panic!("Type error: Expected i32 on the stack");
                }
            },
            Instruction::BrTable(table_data) => (),
            Instruction::Return => {
                let frame = self.function_stack.pop().unwrap();
                self.ip = frame.ret_addr;
            },

            // Calls
            Instruction::Call(index) => {
                let func_type = self.module.function_section().unwrap().entries()[index as usize].type_ref();
                let Function(func_type) = &self.module.type_section().unwrap().types()[func_type as usize];
                let func = &self.module.code_section().unwrap().bodies()[index as usize];
                
                let params_count = func_type.params().len();
                let locals_count = func.locals().len();
                let mut locals = Vec::with_capacity(params_count + locals_count);

                for _ in 0..params_count {
                    locals.push(self.pop()?);
                }
                locals.reverse();

                for local in func.locals() {
                    let default_val = Value::default(local.value_type());
                    for _ in 0..local.count() {
                        locals.push(default_val);
                    }
                }

                self.function_stack.push(FunctionFrame {
                    ret_addr: self.ip,
                    locals
                });

                self.ip = CodePosition { func_index: index as usize, instr_index: 0 };
            },
            Instruction::CallIndirect(signature, _) => (),
            Instruction::Drop => {
                self.pop()?;
            },
            Instruction::Select => (),
            Instruction::GetLocal(index) => {
                let val = self.locals()[index as usize];
                self.push(val);
            },
            Instruction::SetLocal(index) => {
                let val = self.pop()?;
                self.locals()[index as usize] = val;
            },
            Instruction::TeeLocal(index) => {
                let val = self.value_stack.last().ok_or(Trap::PopFromEmptyStack)?;
                self.locals()[index as usize] = *val;
            },
            Instruction::GetGlobal(index) => {
                let val = self.globals[index as usize];
                self.push(val);
            },
            Instruction::SetGlobal(index) => {
                let val = self.pop()?;
                self.globals[index as usize] = val;
            },

            // All store/load instructions operate with 'memory immediates'
            // which represented here as (flag, offset) tuple
            Instruction::I32Load(flag, offset) => (),
            Instruction::I64Load(flag, offset) => (),
            Instruction::F32Load(flag, offset) => (),
            Instruction::F64Load(flag, offset) => (),
            Instruction::I32Load8S(flag, offset) => (),
            Instruction::I32Load8U(flag, offset) => (),
            Instruction::I32Load16S(flag, offset) => (),
            Instruction::I32Load16U(flag, offset) => (),
            Instruction::I64Load8S(flag, offset) => (),
            Instruction::I64Load8U(flag, offset) => (),
            Instruction::I64Load16S(flag, offset) => (),
            Instruction::I64Load16U(flag, offset) => (),
            Instruction::I64Load32S(flag, offset) => (),
            Instruction::I64Load32U(flag, offset) => (),
            Instruction::I32Store(flag, offset) => (),
            Instruction::I64Store(flag, offset) => (),
            Instruction::F32Store(flag, offset) => (),
            Instruction::F64Store(flag, offset) => (),
            Instruction::I32Store8(flag, offset) => (),
            Instruction::I32Store16(flag, offset) => (),
            Instruction::I64Store8(flag, offset) => (),
            Instruction::I64Store16(flag, offset) => (),
            Instruction::I64Store32(flag, offset) => (),

            Instruction::CurrentMemory(_) => (),
            Instruction::GrowMemory(_) => (),

            Instruction::I32Const(val) => self.push(Value::I32(val as u32)),
            Instruction::I64Const(val) => self.push(Value::I64(val as u32)),
            Instruction::F32Const(val) => self.push(Value::F32(f32::from_bits(val))),
            Instruction::F64Const(val) => self.push(Value::F64(f64::from_bits(val))),

            Instruction::I32Eqz => (),
            Instruction::I32Eq => (),
            Instruction::I32Ne => (),
            Instruction::I32LtS => (),
            Instruction::I32LtU => (),
            Instruction::I32GtS => (),
            Instruction::I32GtU => (),
            Instruction::I32LeS => (),
            Instruction::I32LeU => (),
            Instruction::I32GeS => (),
            Instruction::I32GeU => (),

            Instruction::I64Eqz => (),
            Instruction::I64Eq => (),
            Instruction::I64Ne => (),
            Instruction::I64LtS => (),
            Instruction::I64LtU => (),
            Instruction::I64GtS => (),
            Instruction::I64GtU => (),
            Instruction::I64LeS => (),
            Instruction::I64LeU => (),
            Instruction::I64GeS => (),
            Instruction::I64GeU => (),

            Instruction::F32Eq => (),
            Instruction::F32Ne => (),
            Instruction::F32Lt => (),
            Instruction::F32Gt => (),
            Instruction::F32Le => (),
            Instruction::F32Ge => (),

            Instruction::F64Eq => (),
            Instruction::F64Ne => (),
            Instruction::F64Lt => (),
            Instruction::F64Gt => (),
            Instruction::F64Le => (),
            Instruction::F64Ge => (),

            Instruction::I32Clz => (),
            Instruction::I32Ctz => (),
            Instruction::I32Popcnt => (),
            Instruction::I32Add => (),
            Instruction::I32Sub => (),
            Instruction::I32Mul => (),
            Instruction::I32DivS => (),
            Instruction::I32DivU => (),
            Instruction::I32RemS => (),
            Instruction::I32RemU => (),
            Instruction::I32And => (),
            Instruction::I32Or => (),
            Instruction::I32Xor => (),
            Instruction::I32Shl => (),
            Instruction::I32ShrS => (),
            Instruction::I32ShrU => (),
            Instruction::I32Rotl => (),
            Instruction::I32Rotr => (),

            Instruction::I64Clz => (),
            Instruction::I64Ctz => (),
            Instruction::I64Popcnt => (),
            Instruction::I64Add => (),
            Instruction::I64Sub => (),
            Instruction::I64Mul => (),
            Instruction::I64DivS => (),
            Instruction::I64DivU => (),
            Instruction::I64RemS => (),
            Instruction::I64RemU => (),
            Instruction::I64And => (),
            Instruction::I64Or => (),
            Instruction::I64Xor => (),
            Instruction::I64Shl => (),
            Instruction::I64ShrS => (),
            Instruction::I64ShrU => (),
            Instruction::I64Rotl => (),
            Instruction::I64Rotr => (),
            Instruction::F32Abs => (),
            Instruction::F32Neg => (),
            Instruction::F32Ceil => (),
            Instruction::F32Floor => (),
            Instruction::F32Trunc => (),
            Instruction::F32Nearest => (),
            Instruction::F32Sqrt => (),
            Instruction::F32Add => (),
            Instruction::F32Sub => (),
            Instruction::F32Mul => (),
            Instruction::F32Div => (),
            Instruction::F32Min => (),
            Instruction::F32Max => (),
            Instruction::F32Copysign => (),
            Instruction::F64Abs => (),
            Instruction::F64Neg => (),
            Instruction::F64Ceil => (),
            Instruction::F64Floor => (),
            Instruction::F64Trunc => (),
            Instruction::F64Nearest => (),
            Instruction::F64Sqrt => (),
            Instruction::F64Add => (),
            Instruction::F64Sub => (),
            Instruction::F64Mul => (),
            Instruction::F64Div => (),
            Instruction::F64Min => (),
            Instruction::F64Max => (),
            Instruction::F64Copysign => (),

            Instruction::I32WrapI64 => (),
            Instruction::I32TruncSF32 => (),
            Instruction::I32TruncUF32 => (),
            Instruction::I32TruncSF64 => (),
            Instruction::I32TruncUF64 => (),
            Instruction::I64ExtendSI32 => (),
            Instruction::I64ExtendUI32 => (),
            Instruction::I64TruncSF32 => (),
            Instruction::I64TruncUF32 => (),
            Instruction::I64TruncSF64 => (),
            Instruction::I64TruncUF64 => (),
            Instruction::F32ConvertSI32 => (),
            Instruction::F32ConvertUI32 => (),
            Instruction::F32ConvertSI64 => (),
            Instruction::F32ConvertUI64 => (),
            Instruction::F32DemoteF64 => (),
            Instruction::F64ConvertSI32 => (),
            Instruction::F64ConvertUI32 => (),
            Instruction::F64ConvertSI64 => (),
            Instruction::F64ConvertUI64 => (),
            Instruction::F64PromoteF32 => (),

            Instruction::I32ReinterpretF32 => (),
            Instruction::I64ReinterpretF64 => (),
            Instruction::F32ReinterpretI32 => (),
            Instruction::F64ReinterpretI64 => (),

            Instruction::I32Extend8S => (),
            Instruction::I32Extend16S => (),
            Instruction::I64Extend8S => (),
            Instruction::I64Extend16S => (),
            Instruction::I64Extend32S => (),
            _ => self.trap(Trap::UnknownInstruction(instr)),
        }

        if self.function_stack.is_empty() {
            self.trap(Trap::ExecutionFinished);
        }

        if let Some(trap) = &self.trap {
            Err(trap.to_owned())
        }
        else {
            Ok(())
        }
    }
}
