use bwasm;

use super::{eval_init_expr, InitError};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
    pub fn new(table: &bwasm::Table) -> Self {
        let elements = vec![TableElement::Null; table.limits().initial() as usize];
        Table { elements }
    }

    pub fn get(&self, index: u32) -> TableElement {
        self.elements
            .get(index as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn from_module(module: &bwasm::Module) -> Result<Vec<Table>, InitError> {
        let mut tables: Vec<_> = module.tables().iter().map(Table::new).collect();

        for init in module.table_inits() {
            let table = &mut tables[init.index() as usize];
            let offset = eval_init_expr(init.offset())?;
            let offset = match offset.to::<i32>() {
                Some(val) => val as usize,
                None => return Err(InitError::OffsetInvalidType(offset.value_type())),
            };
            for (i, ele) in init.entries().iter().enumerate() {
                let ele = TableElement::Func(*ele);
                let index = i + offset;
                if index >= table.elements.len() {
                    table.elements.push(ele);
                } else {
                    table.elements[index] = ele;
                }
            }
        }

        Ok(tables)
    }
}
