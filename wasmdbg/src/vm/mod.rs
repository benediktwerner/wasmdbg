use bwasm::{InitExpr, ValueType};
use thiserror::Error;

use crate::Value;

mod instance;
mod memory;
mod table;

pub use instance::*;
pub use memory::*;
pub use table::*;

#[derive(Error, Clone, Debug)]
pub enum InitError {
    #[error("Initalizer contains global.get which requires imports (unimplemented)")]
    GlobalGetUnimplemented,
    #[error("Initializer type mismatch. Expected \"{expected}\", found \"{found}\"")]
    MismatchedType { expected: ValueType, found: ValueType },
    #[error("Offset expr has invalid type. Expected \"i32\", found \"{0}\"")]
    OffsetInvalidType(ValueType),
}

#[derive(Error, Clone, Debug)]
pub enum Trap {
    #[error("Reached unreachable")]
    ReachedUnreachable,
    #[error("Pop from empty stack")]
    PopFromEmptyStack,
    #[error("Tried to access function frame but there was none")]
    NoFunctionFrame,
    #[error("Execution finished")]
    ExecutionFinished,
    #[error("Type error. Expected \"{expected}\", found \"{found}\"")]
    TypeError { expected: ValueType, found: ValueType },
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Signed integer overflow")]
    SignedIntegerOverflow,
    #[error("Invalid conversion to integer")]
    InvalidConversionToInt,
    #[error("No table present")]
    NoTable,
    #[error("No memory present")]
    NoMemory,
    #[error("Indirect callee absent (no table or invalid table index)")]
    IndirectCalleeAbsent,
    #[error("Indirect call type mismatch")]
    IndirectCallTypeMismatch,
    #[error("No function with index {0}")]
    NoFunctionWithIndex(u32),
    #[error("No start function")]
    NoStartFunction,
    #[error("Reached breakpoint {0}")]
    BreakpointReached(u32),
    #[error("Reached watchpoint {0}")]
    WatchpointReached(u32),
    #[error("Invalid branch index")]
    InvalidBranchIndex,
    #[error("Out of range memory access at address {0:#08x}")]
    MemoryAccessOutOfRange(u32),
    #[error("Tried to call unsupported imported function: {0}")]
    UnsupportedCallToImportedFunction(u32),
    #[error("Value stack overflow")]
    ValueStackOverflow,
    #[error("Label stack overflow")]
    LabelStackOverflow,
    #[error("Function stack overflow")]
    FunctionStackOverflow,
    #[error("WASI process exited with exitcode {0}")]
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
