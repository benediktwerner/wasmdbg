use bwasm::{InitExpr, MemoryInit, TableInit};

use super::InitError;
use crate::Value;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImportHandler {
    pub global_inits: Vec<Value>,
    pub table_inits: Vec<TableInit>,
    pub memory_inits: Vec<MemoryInit>,
}

impl ImportHandler {
    pub(crate) fn eval_init_expr(&self, init_expr: &InitExpr) -> Result<Value, InitError> {
        match *init_expr {
            InitExpr::I32Const(val) => Ok(Value::from(val)),
            InitExpr::I64Const(val) => Ok(Value::from(val)),
            InitExpr::F32Const(val) => Ok(Value::from(val)),
            InitExpr::F64Const(val) => Ok(Value::from(val)),
            InitExpr::Global(index) => self
                .global_inits
                .get(index as usize)
                .copied()
                .ok_or_else(|| InitError::MissingImportedGlobalInit(index)),
        }
    }
}
