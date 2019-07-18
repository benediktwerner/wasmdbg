extern crate parity_wasm;

use std::cell::RefCell;
use std::rc::Rc;

use parity_wasm::elements::{
    FuncBody, FunctionType, InitExpr, Instruction, Module, ResizableLimits, TableType,
    Type::Function, ValueType,
};

use crate::nan_preserving_float::{F32, F64};
use crate::value::{ExtendTo, Integer, LittleEndianConvert, Number, Value, WrapTo};
use crate::Breakpoints;

#[derive(Debug, Fail)]
pub enum InitError {
    #[fail(display = "Initalizer contains global.get which requires imports (unimplemented)")]
    InitExprGlobalGetUnimplemented,
    #[fail(display = "Initializer contains invalid instruction: {}", _0)]
    InitExprInvalidInstruction(Instruction),
    #[fail(
        display = "Initializer type mismatch. Expected \"{}\", found \"{}\"",
        expected, found
    )]
    InitMismatchedType {
        expected: ValueType,
        found: ValueType,
    },
}

#[derive(Clone, Debug, Fail)]
pub enum Trap {
    #[fail(display = "Reached unreachable")]
    ReachedUnreachable,
    #[fail(display = "Unknown instruction \"{}\"", _0)]
    UnknownInstruction(Instruction),
    #[fail(display = "Pop from empty stack")]
    PopFromEmptyStack,
    #[fail(display = "Tried to access function frame but there was none")]
    NoFunctionFrame,
    #[fail(display = "Execution finished")]
    ExecutionFinished,
    #[fail(display = "Type error. Expected \"{}\", found \"{}\"", expected, found)]
    TypeError {
        expected: ValueType,
        found: ValueType,
    },
    #[fail(display = "Division by zero")]
    DivisionByZero,
    #[fail(display = "Signed integer overflow")]
    SignedIntegerOverflow,
    #[fail(display = "Invalid conversion to integer")]
    InvalidConversionToInt,
    #[fail(display = "No table present")]
    NoTable,
    #[fail(display = "Indirect callee absent (no table or invalid table index)")]
    IndirectCalleeAbsent,
    #[fail(display = "Indirect call type mismatch")]
    IndirectCallTypeMismatch,
    #[fail(display = "No function with index {}", _0)]
    NoFunctionWithIndex(u32),
    #[fail(display = "No start function")]
    NoStartFunction,
    #[fail(display = "Reached breakpoint {}", _0)]
    BreakpointReached(u32),
    #[fail(display = "Invalid branch index")]
    InvalidBranchIndex,
}

pub type VMResult<T> = Result<T, Trap>;

#[derive(Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CodePosition {
    pub func_index: u32,
    pub instr_index: u32,
}

impl CodePosition {
    pub fn new(func_index: u32, instr_index: u32) -> Self {
        CodePosition {
            func_index,
            instr_index,
        }
    }
}

#[derive(Debug)]
pub enum Label {
    Bound(u32),
    Unbound,
    Return,
}

pub struct FunctionFrame {
    pub ret_addr: CodePosition,
    pub locals: Vec<Value>,
}

const PAGE_SIZE: usize = 64 * 1024; // 64 KiB

struct Memory {
    data: Vec<u8>,
    limits: ResizableLimits,
}

impl Memory {
    fn new() -> Memory {
        Memory {
            data: Vec::new(),
            limits: ResizableLimits::new(0, None),
        }
    }

    fn from_module(module: &Module) -> Result<Self, InitError> {
        if let Some(memory_section) = module.memory_section() {
            if let Some(memory) = memory_section.entries().get(0) {
                let limits = *memory.limits();
                let mut data = vec![0; (limits.initial() as usize) * PAGE_SIZE];

                if let Some(data_section) = module.data_section() {
                    for data_segment in data_section.entries() {
                        if data_segment.index() == 0 {
                            let offset = match data_segment.offset() {
                                Some(offset_expr) => {
                                    eval_init_expr(offset_expr)?.expect::<u32>().unwrap_or(0)
                                }
                                None => 0,
                            } as usize;
                            let len = data_segment.value().len();
                            data[offset..offset + len].copy_from_slice(data_segment.value());
                        }
                    }
                }

                return Ok(Memory { data, limits });
            }
        }
        Ok(Self::new())
    }

    pub fn page_count(&self) -> u32 {
        (self.data.len() / PAGE_SIZE) as u32
    }

    fn grow(&mut self, delta: u32) -> u32 {
        // TODO: check if maximum reached and fail (return -1)
        let page_count = self.page_count();
        if let Some(max) = self.limits.maximum() {
            if page_count + delta > max {
                return -1i32 as u32;
            }
        }
        self.data
            .resize((page_count + delta) as usize * PAGE_SIZE, 0);
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
        value.to_little_endian(&mut self.data[address..address + size]);
        Ok(())
    }
}

#[derive(Clone)]
pub enum TableElement {
    Null,
    Func(u32),
}

impl Default for TableElement {
    fn default() -> Self {
        TableElement::Null
    }
}

pub struct Table {
    elements: Vec<TableElement>,
}

impl Table {
    fn new(table_type: TableType) -> Self {
        let elements = vec![TableElement::Null; table_type.limits().initial() as usize];
        Table { elements }
    }

    fn get(&self, index: u32) -> TableElement {
        self.elements
            .get(index as usize)
            .cloned()
            .unwrap_or_default()
    }

    fn from_module(module: &Module) -> Option<Table> {
        if let Some(table_section) = module.table_section() {
            if let Some(default_table_type) = table_section.entries().get(0) {
                return Some(Table::new(*default_table_type));
            }
        }
        None
    }
}

fn eval_init_expr(init_expr: &InitExpr) -> Result<Value, InitError> {
    let val: Value = match &init_expr.code()[0] {
        Instruction::I32Const(val) => (*val).into(),
        Instruction::I64Const(val) => (*val).into(),
        Instruction::F32Const(val) => (*val).into(),
        Instruction::F64Const(val) => (*val).into(),
        Instruction::GetGlobal(_) => return Err(InitError::InitExprGlobalGetUnimplemented),
        instr => return Err(InitError::InitExprInvalidInstruction(instr.clone())),
    };
    Ok(val)
}

pub struct VM {
    module: Rc<Module>,
    memory: Memory,
    table: Option<Table>,
    ip: CodePosition,
    globals: Vec<Value>,
    value_stack: Vec<Value>,
    label_stack: Vec<Label>,
    function_stack: Vec<FunctionFrame>,
    trap: Option<Trap>,
    breakpoints: Rc<RefCell<Breakpoints>>,
}

impl VM {
    pub fn new(module: Rc<Module>, breakpoints: Rc<RefCell<Breakpoints>>) -> Result<VM, InitError> {
        // TODO: Validate module

        let globals = match module.global_section() {
            Some(global_section) => {
                let mut globals = Vec::with_capacity(global_section.entries().len());
                for global in global_section.entries() {
                    let val = eval_init_expr(global.init_expr())?;
                    if val.value_type() != global.global_type().content_type() {
                        return Err(InitError::InitMismatchedType {
                            expected: global.global_type().content_type(),
                            found: val.value_type(),
                        });
                    }
                    globals.push(val);
                }
                globals
            }
            None => Vec::new(),
        };
        let memory = Memory::from_module(&module)?;
        let table = Table::from_module(&module);

        Ok(VM {
            module,
            memory,
            table,
            ip: CodePosition::default(),
            globals,
            value_stack: Vec::new(),
            label_stack: Vec::new(),
            function_stack: Vec::new(),
            trap: None,
            breakpoints,
        })
    }

    pub fn value_stack(&self) -> &Vec<Value> {
        &self.value_stack
    }

    pub fn function_stack(&self) -> &Vec<FunctionFrame> {
        &self.function_stack
    }

    pub fn label_stack(&self) -> &Vec<Label> {
        &self.label_stack
    }

    pub fn trap(&self) -> Option<&Trap> {
        self.trap.as_ref()
    }

    pub fn ip(&self) -> CodePosition {
        self.ip
    }

    fn push(&mut self, val: Value) {
        self.value_stack.push(val);
    }

    fn pop(&mut self) -> VMResult<Value> {
        self.value_stack.pop().ok_or(Trap::PopFromEmptyStack)
    }

    fn pop_expect<T: Number>(&mut self) -> VMResult<T> {
        if let Some(val) = self.value_stack.pop() {
            if let Some(val) = val.expect::<T>() {
                Ok(val)
            } else {
                Err(Trap::TypeError {
                    expected: val.value_type(),
                    found: T::value_type(),
                })
            }
        } else {
            Err(Trap::PopFromEmptyStack)
        }
    }

    fn locals(&mut self) -> VMResult<&mut [Value]> {
        if let Some(frame) = self.function_stack.last_mut() {
            return Ok(&mut frame.locals);
        }
        Err(Trap::NoFunctionFrame)
    }

    fn curr_func(&self) -> VMResult<&FuncBody> {
        self.module
            .get_func(self.ip.func_index)
            .ok_or_else(|| Trap::NoFunctionWithIndex(self.ip.func_index))
    }

    fn default_table(&self) -> VMResult<&Table> {
        self.table.as_ref().ok_or(Trap::NoTable)
    }

    fn branch(&mut self, mut index: u32) -> VMResult<()> {
        self.label_stack
            .truncate(self.label_stack.len() - index as usize);
        match self.label_stack.last().unwrap() {
            Label::Bound(target) => self.ip.instr_index = *target,
            Label::Unbound => {
                index += 1;
                loop {
                    let curr_func_code = self.curr_func()?.code().elements();
                    match curr_func_code[self.ip.instr_index as usize] {
                        Instruction::Block(_) => index += 1,
                        Instruction::Loop(_) => index += 1,
                        Instruction::If(_) => index += 1,
                        Instruction::End => index -= 1,
                        _ => (),
                    }

                    if index == 0 {
                        break;
                    }

                    self.ip.instr_index += 1;
                }
            }
            _ => return Err(Trap::InvalidBranchIndex),
        }
        Ok(())
    }

    fn branch_else(&mut self) -> VMResult<()> {
        let mut index = 1;
        loop {
            let curr_func_code = self.curr_func()?.code().elements();
            match curr_func_code[self.ip.instr_index as usize] {
                Instruction::Block(_) => index += 1,
                Instruction::Loop(_) => index += 1,
                Instruction::If(_) => index += 1,
                Instruction::Else => {
                    if index == 1 {
                        self.ip.instr_index += 1;
                        break;
                    }
                }
                Instruction::End => index -= 1,
                _ => (),
            }

            if index == 0 {
                break;
            }

            self.ip.instr_index += 1;
        }
        Ok(())
    }

    fn perform_load<T: Number + LittleEndianConvert>(&mut self, offset: u32) -> VMResult<()> {
        let address = self.pop_expect::<u32>()? + offset;
        self.push(self.memory.load::<T>(address)?.into());
        Ok(())
    }

    fn perform_load_extend<T: LittleEndianConvert, U: Number>(
        &mut self,
        offset: u32,
    ) -> VMResult<()>
    where
        T: ExtendTo<U>,
    {
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

    fn perform_store_wrap<T: LittleEndianConvert, U: Number>(&mut self, offset: u32) -> VMResult<()>
    where
        U: WrapTo<T>,
    {
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

    fn unop_try<T: Number, R: Number, F: Fn(T) -> VMResult<R>>(&mut self, fun: F) -> VMResult<()> {
        let val: T = self.pop_expect()?;
        self.push(fun(val)?.into());
        Ok(())
    }

    fn binop<T: Number, R: Number, F: Fn(T, T) -> R>(&mut self, fun: F) -> VMResult<()> {
        let b: T = self.pop_expect()?;
        let a: T = self.pop_expect()?;
        self.push(fun(a, b).into());
        Ok(())
    }

    fn binop_try<T: Number, R: Number, F: Fn(T, T) -> VMResult<R>>(
        &mut self,
        fun: F,
    ) -> VMResult<()> {
        let b: T = self.pop_expect()?;
        let a: T = self.pop_expect()?;
        self.push(fun(a, b)?.into());
        Ok(())
    }

    fn call(&mut self, index: u32) -> VMResult<()> {
        let func_type = self
            .module
            .get_func_type(index)
            .ok_or_else(|| Trap::NoFunctionWithIndex(index))?;

        let params_count = func_type.params().len();
        let mut locals = Vec::new();

        for _ in 0..params_count {
            locals.push(self.pop()?);
        }
        locals.reverse();

        let func = self
            .module
            .get_func(index)
            .ok_or_else(|| Trap::NoFunctionWithIndex(index))?;
        for local in func.locals() {
            let default_val = Value::default(local.value_type());
            for _ in 0..local.count() {
                locals.push(default_val);
            }
        }

        self.label_stack.push(Label::Return);
        self.function_stack.push(FunctionFrame {
            ret_addr: self.ip,
            locals,
        });

        self.ip = CodePosition {
            func_index: index,
            instr_index: 0,
        };

        Ok(())
    }

    pub fn run(&mut self) -> Trap {
        if let Some(start_function) = self.module.start_section() {
            self.run_func(start_function)
        } else {
            Trap::NoStartFunction
        }
    }

    pub fn run_func(&mut self, index: u32) -> Trap {
        self.run_func_args(index, &[])
    }

    pub fn run_func_args(&mut self, index: u32, args: &[Value]) -> Trap {
        self.function_stack.clear();
        self.label_stack.clear();
        self.value_stack.clear();
        self.trap = None;
        self.ip = CodePosition::default();
        for arg in args {
            self.push(*arg);
        }
        if let Err(trap) = self.call(index) {
            return trap;
        }
        if let Some(index) = self.breakpoints.borrow().find(self.ip) {
            return Trap::BreakpointReached(index);
        }
        self.continue_execution()
    }

    pub fn continue_execution(&mut self) -> Trap {
        loop {
            if let Err(trap) = self.execute_step() {
                return trap;
            }
        }
    }

    pub fn execute_step_over(&mut self) -> VMResult<()> {
        let curr_frame_index = self.function_stack.len();
        loop {
            self.execute_step()?;
            if curr_frame_index == self.function_stack.len() {
                return Ok(());
            }
        }
    }

    pub fn execute_step(&mut self) -> VMResult<()> {
        if let Some(trap) = &self.trap {
            return Err(trap.to_owned());
        }

        if let Err(trap) = self.execute_step_internal() {
            if let Trap::BreakpointReached(_) = trap {
                return Err(trap);
            }
            self.trap = Some(trap.clone());
            return Err(trap);
        }

        Ok(())
    }

    #[allow(clippy::float_cmp, clippy::redundant_closure)]
    fn execute_step_internal(&mut self) -> VMResult<()> {
        let func = self.module.get_func(self.ip.func_index).unwrap();
        let instr = func.code().elements()[self.ip.instr_index as usize].clone();
        self.ip.instr_index += 1;

        match instr {
            Instruction::Unreachable => return Err(Trap::ReachedUnreachable),
            Instruction::Nop => (),
            Instruction::Block(_) => self.label_stack.push(Label::Unbound),
            Instruction::Loop(_) => self.label_stack.push(Label::Bound(self.ip.instr_index)),
            Instruction::If(_) => {
                self.label_stack.push(Label::Unbound);
                if self.pop_expect::<u32>()? == 0 {
                    self.branch_else()?;
                }
            }
            Instruction::Else => self.branch(0)?,
            Instruction::End => {
                if let Some(Label::Return) = self.label_stack.pop() {
                    if !self.label_stack().is_empty() {
                        let frame = self.function_stack.pop().unwrap();
                        self.ip = frame.ret_addr;
                    }
                }
            }
            Instruction::Br(index) => self.branch(index)?,
            Instruction::BrIf(index) => {
                if self.pop_expect::<u32>()? != 0 {
                    self.branch(index)?;
                }
            }
            Instruction::BrTable(table_data) => {
                let index = self.pop_expect::<u32>()?;
                let depth = table_data
                    .table
                    .get(index as usize)
                    .unwrap_or(&table_data.default);
                self.branch(*depth)?;
            }
            Instruction::Return => {
                let frame = self.function_stack.pop().unwrap();
                self.ip = frame.ret_addr;
            }

            // Calls
            Instruction::Call(index) => self.call(index)?,
            Instruction::CallIndirect(signature, _) => {
                let callee = self.pop_expect::<u32>()?;
                let func_index = match self.default_table()?.get(callee) {
                    TableElement::Func(func_index) => func_index,
                    _ => return Err(Trap::IndirectCalleeAbsent),
                };
                let func_type = self
                    .module
                    .get_func_type_ref(func_index)
                    .ok_or_else(|| Trap::NoFunctionWithIndex(func_index))?;

                if func_type != signature {
                    return Err(Trap::IndirectCallTypeMismatch);
                }

                self.call(func_index)?;
            }
            Instruction::Drop => {
                self.pop()?;
            }
            Instruction::Select => (),
            Instruction::GetLocal(index) => {
                let val = self.locals()?[index as usize];
                self.push(val);
            }
            Instruction::SetLocal(index) => {
                let val = self.pop()?;
                self.locals()?[index as usize] = val;
            }
            Instruction::TeeLocal(index) => {
                let val = self.value_stack.last().ok_or(Trap::PopFromEmptyStack)?;
                self.locals()?[index as usize] = *val;
            }
            Instruction::GetGlobal(index) => {
                let val = self.globals[index as usize];
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
            Instruction::I32Load16S(_flag, offset) => {
                self.perform_load_extend::<i16, u32>(offset)?
            }
            Instruction::I32Load16U(_flag, offset) => {
                self.perform_load_extend::<u16, u32>(offset)?
            }
            Instruction::I64Load8S(_flag, offset) => self.perform_load_extend::<i8, u64>(offset)?,
            Instruction::I64Load8U(_flag, offset) => self.perform_load_extend::<u8, u64>(offset)?,
            Instruction::I64Load16S(_flag, offset) => {
                self.perform_load_extend::<i16, u64>(offset)?
            }
            Instruction::I64Load16U(_flag, offset) => {
                self.perform_load_extend::<u16, u64>(offset)?
            }
            Instruction::I64Load32S(_flag, offset) => {
                self.perform_load_extend::<i32, u64>(offset)?
            }
            Instruction::I64Load32U(_flag, offset) => {
                self.perform_load_extend::<u32, u64>(offset)?
            }

            Instruction::I32Store(_flag, offset) => self.perform_store::<u32>(offset)?,
            Instruction::I64Store(_flag, offset) => self.perform_store::<u64>(offset)?,
            Instruction::F32Store(_flag, offset) => self.perform_store::<F32>(offset)?,
            Instruction::F64Store(_flag, offset) => self.perform_store::<F64>(offset)?,
            Instruction::I32Store8(_flag, offset) => self.perform_store_wrap::<u8, u32>(offset)?,
            Instruction::I32Store16(_flag, offset) => {
                self.perform_store_wrap::<u16, u32>(offset)?
            }
            Instruction::I64Store8(_flag, offset) => self.perform_store_wrap::<u8, u64>(offset)?,
            Instruction::I64Store16(_flag, offset) => {
                self.perform_store_wrap::<u16, u64>(offset)?
            }
            Instruction::I64Store32(_flag, offset) => {
                self.perform_store_wrap::<u32, u64>(offset)?
            }

            Instruction::CurrentMemory(_) => self.push(Value::I32(self.memory.page_count())),
            Instruction::GrowMemory(_) => {
                let delta = self.pop_expect::<u32>()?;
                let result = self.memory.grow(delta);
                self.push(Value::I32(result));
            }

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
            Instruction::I32GeS => self.binop(|a: u32, b: u32| bool_val(a as i32 >= b as i32))?,
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
            Instruction::I64GeS => self.binop(|a: u64, b: u64| bool_val(a as i64 >= b as i64))?,
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

            Instruction::I32Clz => self.unop(|x: u32| x.leading_zeros())?,
            Instruction::I32Ctz => self.unop(|x: u32| x.trailing_zeros())?,
            Instruction::I32Popcnt => self.unop(|x: u32| x.count_ones())?,
            Instruction::I32Add => self.binop(|a: u32, b: u32| a.wrapping_add(b))?,
            Instruction::I32Sub => self.binop(|a: u32, b: u32| a.wrapping_sub(b))?,
            Instruction::I32Mul => self.binop(|a: u32, b: u32| a.wrapping_mul(b))?,
            Instruction::I32DivS => {
                self.binop_try(|a: u32, b: u32| Ok((a as i32).div(b as i32)? as u32))?
            }
            Instruction::I32DivU => self.binop_try(|a: u32, b: u32| a.div(b))?,
            Instruction::I32RemS => {
                self.binop_try(|a: u32, b: u32| Ok((a as i32).rem(b as i32)? as u32))?
            }
            Instruction::I32RemU => self.binop_try(|a: u32, b: u32| a.rem(b))?,
            Instruction::I32And => self.binop(|a: u32, b: u32| a & b)?,
            Instruction::I32Or => self.binop(|a: u32, b: u32| a | b)?,
            Instruction::I32Xor => self.binop(|a: u32, b: u32| a ^ b)?,
            Instruction::I32Shl => self.binop(|a: u32, b: u32| a.wrapping_shl(b))?,
            Instruction::I32ShrS => self.binop(|a: u32, b: u32| (a as i32 >> b) as u32)?,
            Instruction::I32ShrU => self.binop(|a: u32, b: u32| a >> b)?,
            Instruction::I32Rotl => self.binop(|a: u32, b: u32| a.rotate_left(b))?,
            Instruction::I32Rotr => self.binop(|a: u32, b: u32| a.rotate_right(b))?,

            Instruction::I64Clz => self.unop(|x: u64| x.leading_zeros())?,
            Instruction::I64Ctz => self.unop(|x: u64| x.trailing_zeros())?,
            Instruction::I64Popcnt => self.unop(|x: u64| x.count_ones())?,
            Instruction::I64Add => self.binop(|a: u64, b: u64| a.wrapping_add(b))?,
            Instruction::I64Sub => self.binop(|a: u64, b: u64| a.wrapping_sub(b))?,
            Instruction::I64Mul => self.binop(|a: u64, b: u64| a.wrapping_mul(b))?,
            Instruction::I64DivS => {
                self.binop_try(|a: u64, b: u64| Ok((a as i64).div(b as i64)? as u64))?
            }
            Instruction::I64DivU => self.binop_try(|a: u64, b: u64| a.div(b))?,
            Instruction::I64RemS => {
                self.binop_try(|a: u64, b: u64| Ok((a as i64).rem(b as i64)? as u64))?
            }
            Instruction::I64RemU => self.binop_try(|a: u64, b: u64| a.rem(b))?,
            Instruction::I64And => self.binop(|a: u64, b: u64| a & b)?,
            Instruction::I64Or => self.binop(|a: u64, b: u64| a | b)?,
            Instruction::I64Xor => self.binop(|a: u64, b: u64| a ^ b)?,
            Instruction::I64Shl => self.binop(|a: u64, b: u64| a.wrapping_shl(b as u32))?,
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

            Instruction::I32WrapI64 => self.unop(|x: u64| x as u32)?,
            Instruction::I32TruncSF32 => self.unop_try(|x: F32| {
                Ok(x.trunc_to_i32().ok_or(Trap::InvalidConversionToInt)? as u32)
            })?,
            Instruction::I32TruncUF32 => {
                self.unop_try(|x: F32| x.trunc_to_u32().ok_or(Trap::InvalidConversionToInt))?
            }
            Instruction::I32TruncSF64 => self.unop_try(|x: F64| {
                Ok(x.trunc_to_i32().ok_or(Trap::InvalidConversionToInt)? as u32)
            })?,
            Instruction::I32TruncUF64 => {
                self.unop_try(|x: F64| x.trunc_to_u32().ok_or(Trap::InvalidConversionToInt))?
            }
            Instruction::I64ExtendSI32 => self.unop(|x: u32| -> u64 { (x as i32).extend_to() })?,
            Instruction::I64ExtendUI32 => self.unop(|x: u32| -> u64 { x.extend_to() })?,
            Instruction::I64TruncSF32 => self.unop_try(|x: F32| {
                Ok(x.trunc_to_i64().ok_or(Trap::InvalidConversionToInt)? as u64)
            })?,
            Instruction::I64TruncUF32 => {
                self.unop_try(|x: F32| x.trunc_to_u64().ok_or(Trap::InvalidConversionToInt))?
            }
            Instruction::I64TruncSF64 => self.unop_try(|x: F64| {
                Ok(x.trunc_to_i64().ok_or(Trap::InvalidConversionToInt)? as u64)
            })?,
            Instruction::I64TruncUF64 => {
                self.unop_try(|x: F64| x.trunc_to_u64().ok_or(Trap::InvalidConversionToInt))?
            }
            Instruction::F32ConvertSI32 => self.unop(|x: u32| x as i32 as f32)?,
            Instruction::F32ConvertUI32 => self.unop(|x: u32| x as f32)?,
            Instruction::F32ConvertSI64 => self.unop(|x: u64| x as i64 as f32)?,
            Instruction::F32ConvertUI64 => self.unop(|x: u64| x as f32)?,
            Instruction::F32DemoteF64 => self.unop(|x: f64| x as f32)?,
            Instruction::F64ConvertSI32 => self.unop(|x: u32| f64::from(x as i32))?,
            Instruction::F64ConvertUI32 => self.unop(|x: u32| f64::from(x))?,
            Instruction::F64ConvertSI64 => self.unop(|x: u64| x as i64 as f64)?,
            Instruction::F64ConvertUI64 => self.unop(|x: u64| x as f64)?,
            Instruction::F64PromoteF32 => self.unop(|x: f32| f64::from(x))?,

            Instruction::I32ReinterpretF32 => self.unop(|x: F32| x.to_bits())?,
            Instruction::I64ReinterpretF64 => self.unop(|x: F64| x.to_bits())?,
            Instruction::F32ReinterpretI32 => self.unop(F32::from_bits)?,
            Instruction::F64ReinterpretI64 => self.unop(F64::from_bits)?,

            _ => return Err(Trap::UnknownInstruction(instr)),
        }

        if self.label_stack.is_empty() {
            return Err(Trap::ExecutionFinished);
        }

        if let Some(index) = self.breakpoints.borrow().find(self.ip) {
            return Err(Trap::BreakpointReached(index));
        }

        Ok(())
    }
}

#[allow(clippy::match_bool)]
fn bool_val(val: bool) -> u32 {
    match val {
        true => 1,
        false => 0,
    }
}

pub trait ModuleHelper {
    fn calc_func_index(&self, index: u32) -> Option<usize>;
    fn get_func_type_ref(&self, index: u32) -> Option<u32>;
    fn get_func_type(&self, index: u32) -> Option<&FunctionType>;
    fn get_func(&self, index: u32) -> Option<&FuncBody>;
}

impl ModuleHelper for Module {
    fn calc_func_index(&self, index: u32) -> Option<usize> {
        let imported_funcs = self
            .import_section()
            .map(|s| s.entries().len())
            .unwrap_or(0);
        if (index as usize) < imported_funcs {
            return None;
        }
        Some(index as usize - imported_funcs)
    }

    fn get_func_type_ref(&self, index: u32) -> Option<u32> {
        Some(self.function_section()?.entries()[self.calc_func_index(index)?].type_ref())
    }

    fn get_func_type(&self, index: u32) -> Option<&FunctionType> {
        let Function(ref func_type) =
            self.type_section()?.types()[self.get_func_type_ref(index)? as usize];
        Some(func_type)
    }

    fn get_func(&self, index: u32) -> Option<&FuncBody> {
        Some(&self.code_section()?.bodies()[self.calc_func_index(index)?])
    }
}
