extern crate parity_wasm;

use parity_wasm::elements::{Module, Instruction};


#[derive(Clone)]
pub enum Trap {
    ReachedUnreachable,
    UnknownInstruction(Instruction),
    PopFromEmptyStack,
    ExecutionFinished,
}

type VMResult<T> = Result<T, Trap>;

enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

struct CodePosition(usize, usize);
struct Label(usize);
struct FunctionFrame(CodePosition, Vec<Value>);

struct Memory {
    data: Vec<u8>,
}

pub struct VM {
    module: Box<Module>,
    memory: Memory,
    ip: CodePosition,
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
        VM {
            memory: Memory::new(),
            module,
            ip: CodePosition(0, 0),
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
        if let Some(val) = self.value_stack.pop() {
            return Ok(val);
        }
        Err(Trap::PopFromEmptyStack)
    }

    pub fn execute_step(&mut self) -> VMResult<()> {
        let CodePosition(func_index, instr_index) = self.ip;
        let func = &self.module.code_section().unwrap().bodies()[func_index];
        let instr = func.code().elements()[instr_index].clone();
        self.ip.1 += 1;

        match instr {
            Instruction::Unreachable => self.trap(Trap::ReachedUnreachable),
            Instruction::Nop => (),
            Instruction::Block(_) => (),
            Instruction::Loop(_) => (),
            Instruction::If(_) => (),
            Instruction::Else => (),
            Instruction::End => (),
            Instruction::Br(index) => (),
            Instruction::BrIf(index) => (),
            Instruction::BrTable(table_data) => (),
            Instruction::Return => {
                if let Some(FunctionFrame(new_ip, _)) = self.function_stack.pop() {
                    self.ip = new_ip;
                }
                else {
                    self.trap(Trap::ExecutionFinished);
                }
            },

            Instruction::Call(index) => {
                // let mut locals = Vec::new();
                
            },
            Instruction::CallIndirect(signature, _) => (),

            Instruction::Drop => {
                self.pop()?;
            },
            Instruction::Select => (),

            Instruction::GetLocal(index) => (),
            Instruction::SetLocal(index) => (),
            Instruction::TeeLocal(index) => (),
            Instruction::GetGlobal(index) => (),
            Instruction::SetGlobal(index) => (),

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

            Instruction::I32Const(val) => self.push(Value::I32(val)),
            Instruction::I64Const(val) => self.push(Value::I64(val)),
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

        if let Some(trap) = &self.trap {
            Err(trap.to_owned())
        }
        else {
            Ok(())
        }
    }
}
