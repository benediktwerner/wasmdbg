extern crate parity_wasm;

use parity_wasm::elements::{FuncBody, Instruction, Module, Type::Function, ValueType};
use crate::value::{Value, Number, LittleEndianConvert, ExtendTo, WrapTo};
use crate::nan_preserving_float::{F32, F64};


#[derive(Clone)]
pub enum Trap {
    ReachedUnreachable,
    UnknownInstruction(Instruction),
    PopFromEmptyStack,
    ExecutionFinished,
    TypeError(ValueType, ValueType), // Expected, Found
}

type VMResult<T> = Result<T, Trap>;

#[derive(Default, Clone)]
struct CodePosition {
    func_index: usize,
    instr_index: usize,
}

struct Label {
    target_instr_index: Option<usize>,
}

impl Label {
    fn new(target_instr_index: Option<usize>) -> Self {
        Label { target_instr_index }
    }
}

struct FunctionFrame {
    ret_addr: CodePosition,
    locals: Vec<Value>,
}

const PAGE_SIZE: usize = 64 * 1024; // 64 KiB

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
    fn new() -> Memory {
        Memory { data: Vec::new() }
    }

    pub fn page_count(&self) -> u32 {
        (self.data.len() / PAGE_SIZE) as u32
    }

    fn grow(&mut self, delta: u32) -> u32 {
        // TODO: check if maximum reached and fail (return -1)
        let page_count = self.page_count();
        self.data.resize((page_count + delta) as usize * PAGE_SIZE, 0);
        page_count
    }

    fn load<T: LittleEndianConvert>(&self, address: u32) -> VMResult<T> {
        // TODO: check memory access
        let size = core::mem::size_of::<T>();
        let address = address as usize;
        Ok(T::from_little_endian(&self.data[address..address + size]))
    }

    fn store<T: LittleEndianConvert>(&mut self, address: u32, value: T) -> VMResult<()> {
        // TODO: check memory access
        let size = core::mem::size_of::<T>();
        let address = address as usize;
        value.to_little_endian(&mut self.data[address..address+size]);
        Ok(())
    }
}

impl VM {
    pub fn new(module: Box<Module>) -> VM {
        let globals = match module.global_section() {
            Some(global_section) => {
                let mut globals = Vec::with_capacity(global_section.entries().len());
                for global in global_section.entries() {
                    globals.push(Value::default(global.global_type().content_type()));
                }
                globals
            }
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

    fn pop_expect<T: Number>(&mut self) -> VMResult<T> {
        if let Some(val) = self.value_stack.pop() {
            if let Some(val) = val.value_as_any().downcast_ref::<T>() {
                Ok(*val)
            } else {
                Err(Trap::TypeError(val.value_type(), T::value_type()))
            }
        } else {
            Err(Trap::PopFromEmptyStack)
        }
    }

    fn locals(&mut self) -> &mut [Value] {
        &mut self.function_stack.last_mut().unwrap().locals
    }

    fn curr_func(&self) -> &FuncBody {
        &self.module.code_section().unwrap().bodies()[self.ip.func_index]
    }

    fn curr_code(&self) -> &[Instruction] {
        self.curr_func().code().elements()
    }

    fn branch(&mut self, index: u32) {
        self.label_stack
            .truncate(self.label_stack.len() - index as usize);
        match self.label_stack.pop().unwrap().target_instr_index {
            Some(target) => self.ip.instr_index = target,
            None => {
                let mut index = index;
                loop {
                    match self.curr_code()[self.ip.instr_index] {
                        Instruction::Block(_) => index += 1,
                        Instruction::Loop(_) => index += 1,
                        Instruction::End => index -= 1,
                        _ => (),
                    }

                    if index == 0 {
                        break;
                    }
                }
            }
        }
    }

    fn perform_load<T: Number + LittleEndianConvert>(&mut self, offset: u32) -> VMResult<()> {
        let address = self.pop_expect::<u32>()? + offset;
        self.push(self.memory.load::<T>(address)?.into());
        Ok(())
    }

    fn perform_load_extend<T: LittleEndianConvert, U: Number>(&mut self, offset: u32) -> VMResult<()> 
        where T: ExtendTo<U> {
        let address = self.pop_expect::<u32>()? + offset;
        let val: T = self.memory.load(address)?;
        let val: U = val.extend_to();
        self.push(val.into());
        Ok(())
    }

    fn perform_store<T: Number + LittleEndianConvert>(&mut self, offset: u32) -> VMResult<()> {
        let value = self.pop_expect::<T>()?;
        let address = self.pop_expect::<u32>()? + offset;
        self.memory.store(address, value)?;
        Ok(())
    }

    fn perform_store_wrap<T: LittleEndianConvert, U: Number>(&mut self, offset: u32) -> VMResult<()> where U: WrapTo<T> {
        let value: U = self.pop_expect()?;
        let value: T = value.wrap_to();
        let address = self.pop_expect::<u32>()? + offset;
        self.memory.store(address, value)?;
        Ok(())
    }

    fn unop<T: Number, R: Number, F: Fn(T) -> R>(&mut self, fun: F) -> VMResult<()> {
        let val: T = self.pop_expect()?;
        self.push(fun(val).into());
        Ok(())
    }

    fn binop<T: Number, R: Number, F: Fn(T, T) -> R>(&mut self, fun: F) -> VMResult<()> {
        let b: T = self.pop_expect()?;
        let a: T = self.pop_expect()?;
        self.push(fun(a, b).into());
        Ok(())
    }

    #[allow(clippy::float_cmp)]
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
            }
            Instruction::Br(index) => self.branch(index),
            Instruction::BrIf(index) => {
                if self.pop_expect::<u32>()? != 0 {
                    self.branch(index);
                }
            }
            Instruction::BrTable(table_data) => (),
            Instruction::Return => {
                let frame = self.function_stack.pop().unwrap();
                self.ip = frame.ret_addr;
            }

            // Calls
            Instruction::Call(index) => {
                let func_type =
                    self.module.function_section().unwrap().entries()[index as usize].type_ref();
                let Function(func_type) =
                    &self.module.type_section().unwrap().types()[func_type as usize];

                let params_count = func_type.params().len();
                let mut locals = Vec::new();

                for _ in 0..params_count {
                    locals.push(self.pop()?);
                }
                locals.reverse();

                let func = &self.module.code_section().unwrap().bodies()[index as usize];
                for local in func.locals() {
                    let default_val = Value::default(local.value_type());
                    for _ in 0..local.count() {
                        locals.push(default_val.clone());
                    }
                }

                self.function_stack.push(FunctionFrame {
                    ret_addr: self.ip.clone(),
                    locals,
                });

                self.ip = CodePosition {
                    func_index: index as usize,
                    instr_index: 0,
                };
            }
            Instruction::CallIndirect(signature, _) => (),
            Instruction::Drop => {
                self.pop()?;
            }
            Instruction::Select => (),
            Instruction::GetLocal(index) => {
                let val = self.locals()[index as usize].clone();
                self.push(val);
            }
            Instruction::SetLocal(index) => {
                let val = self.pop()?;
                self.locals()[index as usize] = val;
            }
            Instruction::TeeLocal(index) => {
                let val = self
                    .value_stack
                    .last()
                    .ok_or(Trap::PopFromEmptyStack)?
                    .clone();
                self.locals()[index as usize] = val;
            }
            Instruction::GetGlobal(index) => {
                let val = self.globals[index as usize].clone();
                self.push(val);
            }
            Instruction::SetGlobal(index) => {
                let val = self.pop()?;
                self.globals[index as usize] = val;
            }

            // All store/load instructions operate with 'memory immediates'
            // which represented here as (flag, offset) tuple
            Instruction::I32Load(_flag, offset) => self.perform_load::<u32>(offset)?,
            Instruction::I64Load(_flag, offset) => self.perform_load::<u64>(offset)?,
            Instruction::F32Load(_flag, offset) => self.perform_load::<F32>(offset)?,
            Instruction::F64Load(_flag, offset) => self.perform_load::<F64>(offset)?,
            Instruction::I32Load8S(_flag, offset) => self.perform_load_extend::<i8, u32>(offset)?,
            Instruction::I32Load8U(_flag, offset) => self.perform_load_extend::<u8, u32>(offset)?,
            Instruction::I32Load16S(_flag, offset) => self.perform_load_extend::<i16, u32>(offset)?,
            Instruction::I32Load16U(_flag, offset) => self.perform_load_extend::<u16, u32>(offset)?,
            Instruction::I64Load8S(_flag, offset) => self.perform_load_extend::<i8, u64>(offset)?,
            Instruction::I64Load8U(_flag, offset) => self.perform_load_extend::<u8, u64>(offset)?,
            Instruction::I64Load16S(_flag, offset) => self.perform_load_extend::<i16, u64>(offset)?,
            Instruction::I64Load16U(_flag, offset) => self.perform_load_extend::<u16, u64>(offset)?,
            Instruction::I64Load32S(_flag, offset) => self.perform_load_extend::<i32, u64>(offset)?,
            Instruction::I64Load32U(_flag, offset) => self.perform_load_extend::<u32, u64>(offset)?,

            Instruction::I32Store(_flag, offset) => self.perform_store::<u32>(offset)?,
            Instruction::I64Store(_flag, offset) => self.perform_store::<u64>(offset)?,
            Instruction::F32Store(_flag, offset) => self.perform_store::<F32>(offset)?,
            Instruction::F64Store(_flag, offset) => self.perform_store::<F64>(offset)?,
            Instruction::I32Store8(_flag, offset) => self.perform_store_wrap::<u8, u32>(offset)?,
            Instruction::I32Store16(_flag, offset) => self.perform_store_wrap::<u16, u32>(offset)?,
            Instruction::I64Store8(_flag, offset) => self.perform_store_wrap::<u8, u64>(offset)?,
            Instruction::I64Store16(_flag, offset) => self.perform_store_wrap::<u16, u64>(offset)?,
            Instruction::I64Store32(_flag, offset) => self.perform_store_wrap::<u32, u64>(offset)?,

            Instruction::CurrentMemory(_) => self.push(Value::I32(self.memory.page_count())),
            Instruction::GrowMemory(_) => {
                let delta = self.pop_expect::<u32>()?;
                let result = self.memory.grow(delta);
                self.push(Value::I32(result));
            },

            Instruction::I32Const(val) => self.push(Value::I32(val as u32)),
            Instruction::I64Const(val) => self.push(Value::I64(val as u64)),
            Instruction::F32Const(val) => self.push(Value::F32(F32::from_bits(val))),
            Instruction::F64Const(val) => self.push(Value::F64(F64::from_bits(val))),

            Instruction::I32Eqz => self.unop(|x: u32| bool_val(x == 0))?,
            Instruction::I32Eq => self.binop(|a: u32, b: u32| bool_val(a == b))?,
            Instruction::I32Ne => self.binop(|a: u32, b: u32| bool_val(a != b))?,
            Instruction::I32LtS => self.binop(|a: u32, b: u32| bool_val(a as i32 == b as i32))?,
            Instruction::I32LtU => self.binop(|a: u32, b: u32| bool_val(a == b))?,
            Instruction::I32GtS => self.binop(|a: u32, b: u32| bool_val(a as i32 > b as i32))?,
            Instruction::I32GtU => self.binop(|a: u32, b: u32| bool_val(a > b))?,
            Instruction::I32LeS => self.binop(|a: u32, b: u32| bool_val(a as i32 <= b as i32))?,
            Instruction::I32LeU => self.binop(|a: u32, b: u32| bool_val(a <= b))?,
            Instruction::I32GeS => self.binop(|a: u32, b: u32| bool_val(a as i32>= b as i32))?,
            Instruction::I32GeU => self.binop(|a: u32, b: u32| bool_val(a >= b))?,

            Instruction::I64Eqz => self.unop(|x: u64| bool_val(x == 0))?,
            Instruction::I64Eq => self.binop(|a: u64, b: u64| bool_val(a == b))?,
            Instruction::I64Ne => self.binop(|a: u64, b: u64| bool_val(a != b))?,
            Instruction::I64LtS => self.binop(|a: u64, b: u64| bool_val(a as i64 == b as i64))?,
            Instruction::I64LtU => self.binop(|a: u64, b: u64| bool_val(a == b))?,
            Instruction::I64GtS => self.binop(|a: u64, b: u64| bool_val(a as i64 > b as i64))?,
            Instruction::I64GtU => self.binop(|a: u64, b: u64| bool_val(a > b))?,
            Instruction::I64LeS => self.binop(|a: u64, b: u64| bool_val(a as i64 <= b as i64))?,
            Instruction::I64LeU => self.binop(|a: u64, b: u64| bool_val(a <= b))?,
            Instruction::I64GeS => self.binop(|a: u64, b: u64| bool_val(a as i64>= b as i64))?,
            Instruction::I64GeU => self.binop(|a: u64, b: u64| bool_val(a >= b))?,

            Instruction::F32Eq => self.binop(|a: F32, b: F32| bool_val(a == b))?,
            Instruction::F32Ne => self.binop(|a: F32, b: F32| bool_val(a != b))?,
            Instruction::F32Lt => self.binop(|a: F32, b: F32| bool_val(a < b))?,
            Instruction::F32Gt => self.binop(|a: F32, b: F32| bool_val(a > b))?,
            Instruction::F32Le => self.binop(|a: F32, b: F32| bool_val(a <= b))?,
            Instruction::F32Ge => self.binop(|a: F32, b: F32| bool_val(a >= b))?,

            Instruction::F64Eq => self.binop(|a: F64, b: F64| bool_val(a == b))?,
            Instruction::F64Ne => self.binop(|a: F64, b: F64| bool_val(a != b))?,
            Instruction::F64Lt => self.binop(|a: F64, b: F64| bool_val(a < b))?,
            Instruction::F64Gt => self.binop(|a: F64, b: F64| bool_val(a > b))?,
            Instruction::F64Le => self.binop(|a: F64, b: F64| bool_val(a <= b))?,
            Instruction::F64Ge => self.binop(|a: F64, b: F64| bool_val(a >= b))?,

            Instruction::I32Clz => (),
            Instruction::I32Ctz => (),
            Instruction::I32Popcnt => (),
            Instruction::I32Add => self.binop(|a: u32, b: u32| a + b)?,
            Instruction::I32Sub => self.binop(|a: u32, b: u32| a - b)?,
            Instruction::I32Mul => self.binop(|a: u32, b: u32| a * b)?,
            Instruction::I32DivS => (),
            Instruction::I32DivU => (),
            Instruction::I32RemS => (),
            Instruction::I32RemU => (),
            Instruction::I32And => self.binop(|a: u32, b: u32| a & b)?,
            Instruction::I32Or => self.binop(|a: u32, b: u32| a | b)?,
            Instruction::I32Xor => self.binop(|a: u32, b: u32| a ^ b)?,
            Instruction::I32Shl => self.binop(|a: u32, b: u32| a << b)?,
            Instruction::I32ShrS => self.binop(|a: u32, b: u32| (a as i32 >> b) as u32)?,
            Instruction::I32ShrU => self.binop(|a: u32, b: u32| a >> b)?,
            Instruction::I32Rotl => self.binop(|a: u32, b: u32| a.rotate_left(b))?,
            Instruction::I32Rotr => self.binop(|a: u32, b: u32| a.rotate_right(b))?,

            Instruction::I64Clz => (),
            Instruction::I64Ctz => (),
            Instruction::I64Popcnt => (),
            Instruction::I64Add => self.binop(|a: u64, b: u64| a + b)?,
            Instruction::I64Sub => self.binop(|a: u64, b: u64| a - b)?,
            Instruction::I64Mul => self.binop(|a: u64, b: u64| a * b)?,
            Instruction::I64DivS => (),
            Instruction::I64DivU => (),
            Instruction::I64RemS => (),
            Instruction::I64RemU => (),
            Instruction::I64And => self.binop(|a: u64, b: u64| a & b)?,
            Instruction::I64Or => self.binop(|a: u64, b: u64| a | b)?,
            Instruction::I64Xor => self.binop(|a: u64, b: u64| a ^ b)?,
            Instruction::I64Shl => self.binop(|a: u64, b: u64| a << b)?,
            Instruction::I64ShrS => self.binop(|a: u64, b: u64| (a as i64 >> b) as u64)?,
            Instruction::I64ShrU => self.binop(|a: u64, b: u64| a >> b)?,
            Instruction::I64Rotl => self.binop(|a: u64, b: u64| a.rotate_left(b as u32))?,
            Instruction::I64Rotr => self.binop(|a: u64, b: u64| a.rotate_right(b as u32))?,

            Instruction::F32Abs => self.unop(|x: F32| x.abs())?,
            Instruction::F32Neg => self.unop(|x: F32| -x)?,
            Instruction::F32Ceil => self.unop(|x: F32| x.ceil())?,
            Instruction::F32Floor => self.unop(|x: F32| x.floor())?,
            Instruction::F32Trunc => self.unop(|x: F32| x.trunc())?,
            Instruction::F32Nearest => self.unop(|x: F32| x.nearest())?,
            Instruction::F32Sqrt => self.unop(|x: F32| x.sqrt())?,
            Instruction::F32Add => self.binop(|a: F32, b: F32| a + b)?,
            Instruction::F32Sub => self.binop(|a: F32, b: F32| a - b)?,
            Instruction::F32Mul => self.binop(|a: F32, b: F32| a * b)?,
            Instruction::F32Div => self.binop(|a: F32, b: F32| a / b)?,
            Instruction::F32Min => self.binop(|a: F32, b: F32| a.min(b))?,
            Instruction::F32Max => self.binop(|a: F32, b: F32| a.max(b))?,
            Instruction::F32Copysign => self.binop(|a: F32, b: F32| a.copysign(b))?,

            Instruction::F64Abs => self.unop(|x: F64| x.abs())?,
            Instruction::F64Neg => self.unop(|x: F64| -x)?,
            Instruction::F64Ceil => self.unop(|x: F64| x.ceil())?,
            Instruction::F64Floor => self.unop(|x: F64| x.floor())?,
            Instruction::F64Trunc => self.unop(|x: F64| x.trunc())?,
            Instruction::F64Nearest => self.unop(|x: F64| x.nearest())?,
            Instruction::F64Sqrt => self.unop(|x: F64| x.sqrt())?,
            Instruction::F64Add => self.binop(|a: F64, b: F64| a + b)?,
            Instruction::F64Sub => self.binop(|a: F64, b: F64| a - b)?,
            Instruction::F64Mul => self.binop(|a: F64, b: F64| a * b)?,
            Instruction::F64Div => self.binop(|a: F64, b: F64| a / b)?,
            Instruction::F64Min => self.binop(|a: F64, b: F64| a.min(b))?,
            Instruction::F64Max => self.binop(|a: F64, b: F64| a.max(b))?,
            Instruction::F64Copysign => self.binop(|a: F64, b: F64| a.copysign(b))?,

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
        } else {
            Ok(())
        }
    }
}

fn bool_val(val: bool) -> u32 {
    match val {
        true => 1,
        false => 0,
    }
}
