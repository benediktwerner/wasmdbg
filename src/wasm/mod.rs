use std::fmt;
use std::iter::{self, FromIterator};
use std::path::Path;

use failure::Fail;
use parity_wasm::elements as pwasm;
pub use parity_wasm::elements::{
    CustomSection, ExportEntry, External, GlobalType, ImportEntry, Instruction, MemoryType,
    ResizableLimits, TableType, ValueType,
};
pub use parity_wasm::SerializationError;

use crate::value::Value;

pub const PAGE_SIZE: u32 = 64 * 1024; // 64 KiB

#[derive(Debug, Fail)]
pub enum LoadError {
    #[fail(display = "File not found")]
    FileNotFound,
    #[fail(display = "Error while loading file: {}", _0)]
    SerializationError(#[fail(cause)] SerializationError),
}

#[derive(Clone)]
pub struct FunctionType {
    type_ref: u32,
    params: Vec<ValueType>,
    return_type: Option<ValueType>,
}

impl FunctionType {
    fn new(type_ref: u32, func_type: &pwasm::FunctionType) -> Self {
        FunctionType {
            type_ref,
            params: Vec::from(func_type.params()),
            return_type: func_type.return_type(),
        }
    }
    pub fn type_ref(&self) -> u32 {
        self.type_ref
    }
    pub fn params(&self) -> &[ValueType] {
        &self.params
    }
    pub fn return_type(&self) -> Option<ValueType> {
        self.return_type
    }
}

impl fmt::Display for FunctionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params = self
            .params
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        let return_type = match self.return_type {
            Some(return_type) => return_type.to_string(),
            None => String::from("()"),
        };
        write!(f, "fn ({}) -> {}", params, return_type)
    }
}

pub struct Function {
    name: String,
    func_type: FunctionType,
    is_imported: bool,
    locals: Vec<ValueType>,
    instructions: Vec<Instruction>,
}

impl Function {
    fn new(
        name: String,
        func_type: FunctionType,
        locals: Vec<ValueType>,
        instructions: Vec<Instruction>,
    ) -> Self {
        Function {
            name,
            func_type,
            is_imported: false,
            locals,
            instructions,
        }
    }

    fn new_imported(name: String, func_type: FunctionType) -> Self {
        Function {
            name,
            func_type,
            is_imported: true,
            locals: Vec::with_capacity(0),
            instructions: Vec::with_capacity(0),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn func_type(&self) -> &FunctionType {
        &self.func_type
    }
    pub fn is_imported(&self) -> bool {
        self.is_imported
    }
    pub fn locals(&self) -> &[ValueType] {
        &self.locals
    }
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fn {}{}", self.name, &self.func_type.to_string()[3..])
    }
}

pub enum InitExpr {
    Const(Value),
    Global(u32),
}

impl From<&pwasm::InitExpr> for InitExpr {
    fn from(init_expr: &pwasm::InitExpr) -> Self {
        let instrs = init_expr.code();
        assert!(
            instrs.len() == 2,
            "Init expr has invalid length: {}",
            instrs.len()
        );
        assert!(
            instrs[1] == Instruction::End,
            "Init expr has multiple instructions"
        );
        match &instrs[0] {
            Instruction::I32Const(val) => InitExpr::Const((*val).into()),
            Instruction::I64Const(val) => InitExpr::Const((*val).into()),
            Instruction::F32Const(val) => InitExpr::Const((*val).into()),
            Instruction::F64Const(val) => InitExpr::Const((*val).into()),
            Instruction::GetGlobal(index) => InitExpr::Global(*index),
            other => panic!("Invalid instruction in init expr: {}", other),
        }
    }
}

pub struct Global {
    imported: bool,
    global_type: GlobalType,
    init_expr: InitExpr,
}

impl Global {
    pub fn imported(&self) -> bool {
        self.imported
    }
    pub fn global_type(&self) -> &GlobalType {
        &self.global_type
    }
    pub fn init_expr(&self) -> &InitExpr {
        &self.init_expr
    }
}

impl From<&pwasm::GlobalEntry> for Global {
    fn from(global: &pwasm::GlobalEntry) -> Self {
        Global {
            imported: false,
            global_type: *global.global_type(),
            init_expr: global.init_expr().into(),
        }
    }
}

pub struct ElementSegment {
    index: u32,
    offset: InitExpr,
    members: Vec<u32>,
}

impl ElementSegment {
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn offset(&self) -> &InitExpr {
        &self.offset
    }
    pub fn members(&self) -> &[u32] {
        &self.members
    }
}

impl From<&pwasm::ElementSegment> for ElementSegment {
    fn from(seg: &pwasm::ElementSegment) -> Self {
        ElementSegment {
            index: seg.index(),
            offset: seg.offset().as_ref().unwrap().into(),
            members: Vec::from(seg.members()),
        }
    }
}

pub struct DataSegment {
    index: u32,
    offset: InitExpr,
    value: Vec<u8>,
}

impl DataSegment {
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn offset(&self) -> &InitExpr {
        &self.offset
    }
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

impl From<&pwasm::DataSegment> for DataSegment {
    fn from(seg: &pwasm::DataSegment) -> Self {
        DataSegment {
            index: seg.index(),
            offset: seg.offset().as_ref().unwrap().into(),
            value: Vec::from(seg.value()),
        }
    }
}

pub struct Module {
    types: Vec<FunctionType>,
    imports: Vec<ImportEntry>,
    exports: Vec<ExportEntry>,
    functions: Vec<Function>,
    globals: Vec<Global>,
    tables: Vec<TableType>,
    memories: Vec<MemoryType>,
    element_entries: Vec<ElementSegment>,
    data_entries: Vec<DataSegment>,
    start_func: Option<u32>,
    custom_sections: Vec<CustomSection>,
}

impl Module {
    pub fn from_file(file_path: &str) -> Result<Self, LoadError> {
        if !Path::new(file_path).exists() {
            return Err(LoadError::FileNotFound);
        }

        match parity_wasm::deserialize_file(file_path) {
            Ok(module) => Ok(Module::from_parity_module(module)),
            Err(error) => Err(LoadError::SerializationError(error)),
        }
    }

    fn from_parity_module(module: parity_wasm::elements::Module) -> Self {
        let module = match module.parse_names() {
            Ok(module) => module,
            Err((_, module)) => module,
        };

        let mut types = Vec::new();
        if let Some(type_sec) = module.type_section() {
            for (i, t) in type_sec.types().iter().enumerate() {
                let pwasm::Type::Function(func_type) = t;
                types.push(FunctionType::new(i as u32, func_type));
            }
        }

        let mut globals = Vec::new();
        if let Some(global_sec) = module.global_section() {
            for global in global_sec.entries() {
                globals.push(global.into());
            }
        }

        let mut element_entries = Vec::new();
        if let Some(element_sec) = module.elements_section() {
            for entry in element_sec.entries() {
                element_entries.push(entry.into());
            }
        }

        let mut data_entries = Vec::new();
        if let Some(data_sec) = module.data_section() {
            for entry in data_sec.entries() {
                data_entries.push(entry.into());
            }
        }

        let func_count = module
            .function_section()
            .map(|sec| sec.entries().len())
            .unwrap_or(0);
        let mut functions = Vec::with_capacity(func_count);

        if let Some(import_sec) = module.import_section() {
            for entry in import_sec.entries() {
                if let pwasm::External::Function(type_ref) = entry.external() {
                    let name = format!("{}.{}", entry.module(), entry.field());
                    let func_type = types[*type_ref as usize].clone();
                    functions.push(Function::new_imported(name, func_type))
                }
            }
        }

        if let Some(func_sec) = module.function_section() {
            let func_bodies = module.code_section().map(|sec| sec.bodies()).unwrap_or(&[]);
            for (type_ref, body) in func_sec.entries().iter().zip(func_bodies.iter()) {
                let type_ref = type_ref.type_ref();
                let name = format!("f{}", functions.len());
                let func_type = types[type_ref as usize].clone();
                let locals = body
                    .locals()
                    .iter()
                    .flat_map(|locals| {
                        iter::repeat(locals.value_type()).take(locals.count() as usize)
                    })
                    .collect();
                let instructions = Vec::from(body.code().elements());
                functions.push(Function::new(name, func_type, locals, instructions));
            }
        }

        Module {
            types,
            imports: Vec::from(
                module
                    .import_section()
                    .map(|sec| sec.entries())
                    .unwrap_or(&[]),
            ),
            exports: Vec::from(
                module
                    .export_section()
                    .map(|sec| sec.entries())
                    .unwrap_or(&[]),
            ),
            functions,
            globals,
            tables: Vec::from(
                module
                    .table_section()
                    .map(|sec| sec.entries())
                    .unwrap_or(&[]),
            ),
            memories: Vec::from(
                module
                    .memory_section()
                    .map(|sec| sec.entries())
                    .unwrap_or(&[]),
            ),
            element_entries,
            data_entries,
            start_func: module.start_section(),
            custom_sections: Vec::from_iter(module.custom_sections().cloned()),
        }
    }

    pub fn types(&self) -> &[FunctionType] {
        &self.types
    }

    pub fn imports(&self) -> &[ImportEntry] {
        &self.imports
    }

    pub fn exports(&self) -> &[ExportEntry] {
        &self.exports
    }

    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    pub fn get_func(&self, index: u32) -> Option<&Function> {
        self.functions.get(index as usize)
    }

    pub fn globals(&self) -> &[Global] {
        &self.globals
    }

    pub fn tables(&self) -> &[TableType] {
        &self.tables
    }

    pub fn memories(&self) -> &[MemoryType] {
        &self.memories
    }

    pub fn element_entries(&self) -> &[ElementSegment] {
        &self.element_entries
    }

    pub fn data_entries(&self) -> &[DataSegment] {
        &self.data_entries
    }

    pub fn start_func(&self) -> Option<u32> {
        self.start_func
    }

    pub fn custom_sections(&self) -> &[CustomSection] {
        &self.custom_sections
    }
}
