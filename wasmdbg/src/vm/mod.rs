use bwasm::{InitExpr, ValueType};

use crate::Value;

mod instance;
mod memory;
mod table;

pub use instance::*;
pub use memory::*;
pub use table::*;

#[derive(Clone, Debug, Fail)]
pub enum InitError {
    #[fail(display = "Initalizer contains global.get which requires imports (unimplemented)")]
    GlobalGetUnimplemented,
    #[fail(
        display = "Initializer type mismatch. Expected \"{}\", found \"{}\"",
        expected, found
    )]
    MismatchedType {
        expected: ValueType,
        found: ValueType,
    },
    #[fail(
        display = "Offset expr has invalid type. Expected \"i32\", found \"{}\"",
        _0
    )]
    OffsetInvalidType(ValueType),
}

#[derive(Clone, Debug, Fail)]
pub enum Trap {
    #[fail(display = "Reached unreachable")]
    ReachedUnreachable,
    // #[fail(display = "Unknown instruction \"{}\"", _0)]
    // UnknownInstruction(Instruction),
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
    #[fail(display = "No memory present")]
    NoMemory,
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
    #[fail(display = "Reached watchpoint {}", _0)]
    WatchpointReached(u32),
    #[fail(display = "Invalid branch index")]
    InvalidBranchIndex,
    #[fail(display = "Out of range memory access at address {:#08x}", _0)]
    MemoryAccessOutOfRange(u32),
    #[fail(display = "Tried to call unsupported imported function: {}", _0)]
    UnsupportedCallToImportedFunction(u32),
    #[fail(display = "Value stack overflow")]
    ValueStackOverflow,
    #[fail(display = "Label stack overflow")]
    LabelStackOverflow,
    #[fail(display = "Function stack overflow")]
    FunctionStackOverflow,
    #[fail(display = "WASI process exited with exitcode {}", _0)]
    WasiExit(u32),
}

pub type VMResult<T> = Result<T, Trap>;

#[derive(Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CodePosition {
    pub func_index: u32,
    pub instr_index: u32,
}

impl CodePosition {
    pub const fn new(func_index: u32, instr_index: u32) -> Self {
        CodePosition {
            func_index,
            instr_index,
        }
    }
}

impl std::fmt::Display for CodePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.func_index, self.instr_index)
    }
}

fn eval_init_expr(init_expr: &InitExpr) -> Result<Value, InitError> {
    let val = match init_expr {
        InitExpr::I32Const(val) => Value::from(*val),
        InitExpr::I64Const(val) => Value::from(*val),
        InitExpr::F32Const(val) => Value::from(*val),
        InitExpr::F64Const(val) => Value::from(*val),
        InitExpr::Global(_) => return Err(InitError::GlobalGetUnimplemented),
    };
    Ok(val)
}
