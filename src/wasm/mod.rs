use std::fmt;
use std::iter::{self, FromIterator};
use std::path::Path;
use std::process;

use failure::Fail;
use parity_wasm::elements as pwasm;
pub use parity_wasm::elements::{
    CustomSection, ExportEntry, External, GlobalType, ImportEntry, Instruction, Internal,
    MemoryType, ResizableLimits, TableType, ValueType,
};
pub use parity_wasm::SerializationError;

use crate::value::Value;
use crate::wasi::WasiFunction;

pub const PAGE_SIZE: u32 = 64 * 1024; // 64 KiB

#[derive(Debug, Fail)]
pub enum LoadError {
    #[fail(display = "File not found")]
    FileNotFound,
    #[fail(display = "Error while loading file: {}", _0)]
    SerializationError(#[fail(cause)] SerializationError),
    #[fail(display = "Error while validating file: {}", _0)]
    ValidationError(String),
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
    wasi_function: Option<WasiFunction>,
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
            wasi_function: None,
            locals,
            instructions,
        }
    }

    fn new_imported(name: String, func_type: FunctionType) -> Self {
        let wasi_function = if name.starts_with("wasi_unstable.") {
            // TODO: Check type
            WasiFunction::from_name(&name["wasi_unstable.".len()..])
        } else {
            None
        };
        Function {
            name,
            func_type,
            is_imported: true,
            wasi_function,
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
    pub fn wasi_function(&self) -> Option<WasiFunction> {
        self.wasi_function
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
    name: String,
    is_imported: bool,
    is_mutable: bool,
    value_type: ValueType,
    init_expr: InitExpr,
}

impl Global {
    fn from_parity(name: String, global: &pwasm::GlobalEntry) -> Self {
        let global_type = global.global_type();
        Global {
            name,
            is_imported: false,
            is_mutable: global_type.is_mutable(),
            value_type: global_type.content_type(),
            init_expr: global.init_expr().into(),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_imported(&self) -> bool {
        self.is_imported
    }
    pub fn is_mutable(&self) -> bool {
        self.is_mutable
    }
    pub fn value_type(&self) -> ValueType {
        self.value_type
    }
    pub fn init_expr(&self) -> &InitExpr {
        &self.init_expr
    }
}

impl fmt::Display for Global {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let const_str = if self.is_mutable() { "mut  " } else { "const" };
        let init_str = match self.init_expr() {
            InitExpr::Const(val) => format!("{}", val),
            InitExpr::Global(index) => format!("global {}", index),
        };
        write!(f, "{} {:15} = {}", const_str, self.name(), init_str)
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

        // TODO: Do proper validation of the LOADED module
        match process::Command::new("wasm-validate")
            .arg(file_path)
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    return Err(LoadError::ValidationError(
                        String::from_utf8(output.stderr)
                            .expect("Error while reading \"wasm-validate\" output"),
                    ));
                }
            }
            Err(error) => {
                if let std::io::ErrorKind::NotFound = error.kind() {
                    println!(
                        "Could not validate the module because \"wasm-validate\" was not found."
                    );
                    println!("Install \"wabt\" to enable module validation.")
                } else {
                    return Err(LoadError::ValidationError(error.to_string()));
                }
            }
        };

        match parity_wasm::deserialize_file(file_path) {
            Ok(module) => Ok(Module::from_parity_module(module)),
            Err(error) => Err(LoadError::SerializationError(error)),
        }
    }

    fn from_parity_module(module: parity_wasm::elements::Module) -> Self {
        // TODO: What happens when multiple functions have the same name?
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
            for (i, global) in global_sec.entries().iter().enumerate() {
                let name = format!("g{}", i);
                globals.push(Global::from_parity(name, global));
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
                } else {
                    println!("Unsupported import: {:?}", entry);
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

        let imports = Vec::from(
            module
                .import_section()
                .map(|sec| sec.entries())
                .unwrap_or(&[]),
        );
        let exports = Vec::from(
            module
                .export_section()
                .map(|sec| sec.entries())
                .unwrap_or(&[]),
        );
        let tables = Vec::from(
            module
                .table_section()
                .map(|sec| sec.entries())
                .unwrap_or(&[]),
        );
        let memories = Vec::from(
            module
                .memory_section()
                .map(|sec| sec.entries())
                .unwrap_or(&[]),
        );

        for export in &exports {
            match export.internal() {
                Internal::Function(index) => {
                    functions[*index as usize].name = export.field().to_string()
                }
                Internal::Global(index) => {
                    globals[*index as usize].name = export.field().to_string()
                }
                _ => (),
            }
        }

        if let Some(name_sec) = module.names_section() {
            if let Some(func_names) = name_sec.functions() {
                for (i, name) in func_names.names() {
                    functions[i as usize].name = name.clone();
                }
            }
        }

        let start_func = module.start_section().or_else(|| {
            for export in &exports {
                if export.field() == "_start" {
                    if let Internal::Function(index) = export.internal() {
                        let index = *index;
                        let func_type = functions[index as usize].func_type();
                        if func_type.params().is_empty() && func_type.return_type().is_none() {
                            return Some(index);
                        }
                    }
                }
            }
            None
        });

        Module {
            types,
            imports,
            exports,
            functions,
            globals,
            tables,
            memories,
            element_entries,
            data_entries,
            start_func,
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
