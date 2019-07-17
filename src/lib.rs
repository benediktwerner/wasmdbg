#[macro_use]
extern crate failure;
extern crate parity_wasm;


use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use parity_wasm::{elements::Module, SerializationError};

pub mod nan_preserving_float;
pub mod value;
pub mod vm;
use value::Value;
use vm::{CodePosition, InitError, Trap, VM};


#[derive(Debug, Fail)]
pub enum LoadError {
    #[fail(display = "File not found")]
    FileNotFound,
    #[fail(display = "Serialization failed: {}", _0)]
    SerializationError(#[fail(cause)] SerializationError),
}

#[derive(Debug, Fail)]
pub enum DebuggerError {
    #[fail(display = "Failed to initialize wasm instance: {}", _0)]
    InitError(#[fail(cause)] InitError),
    #[fail(display = "No binary file loaded")]
    NoFileLoaded,
    #[fail(display = "The binary is not being run")]
    NoRunningInstance,
    #[fail(display = "Invalid brekapoint position")]
    InvalidBreakpointPosition,
    #[fail(display = "This feature is still unimplemented")]
    Unimplemented,
}

pub type DebuggerResult<T> = Result<T, DebuggerError>;

pub struct Breakpoints {
    breakpoints: HashSet<CodePosition>,
    breakpoint_indices: HashMap<u32, CodePosition>,
    next_breakpoint_index: u32,
}

impl Breakpoints {
    fn new() -> Self {
        Breakpoints {
            breakpoints: HashSet::new(),
            breakpoint_indices: HashMap::new(),
            next_breakpoint_index: 0,
        }
    }

    pub fn find(&self, pos: &CodePosition) -> Option<u32> {
        if self.breakpoints.contains(pos) {
            for (index, breakpoint) in self.breakpoint_indices.iter() {
                if breakpoint == pos {
                    return Some(*index);
                }
            }
        }
        None
    }

    fn add_breakpoint(&mut self, breakpoint: CodePosition) {
        self.breakpoints.insert(breakpoint.clone());
        self.breakpoint_indices
            .insert(self.next_breakpoint_index, breakpoint);
        self.next_breakpoint_index += 1;
    }

    fn delete_breakpoint(&mut self, index: u32) -> bool {
        if let Some(breakpoint) = self.breakpoint_indices.get(&index) {
            self.breakpoints.remove(breakpoint);
            self.breakpoint_indices.remove(&index);
            return true;
        }
        false
    }
}

pub struct File {
    file_path: String,
    module: Rc<Module>,
    breakpoints: Rc<RefCell<Breakpoints>>,
}

#[derive(Default)]
pub struct Debugger {
    file: Option<File>,
    vm: Option<VM>,
}

impl File {
    pub fn file_path(&self) -> &String {
        &self.file_path
    }

    pub fn module(&self) -> &Module {
        &self.module
    }
}

impl Debugger {
    pub fn new() -> Self {
        Debugger {
            file: None,
            vm: None,
        }
    }

    pub fn file(&self) -> Option<&File> {
        self.file.as_ref()
    }

    pub fn module(&self) -> Option<&Module> {
        Some(self.file.as_ref()?.module())
    }

    pub fn vm(&self) -> Option<&VM> {
        self.vm.as_ref()
    }

    pub fn load_file(&mut self, file_path: &str) -> Result<(), LoadError> {
        if !Path::new(file_path).exists() {
            return Err(LoadError::FileNotFound);
        }

        let module =
            parity_wasm::deserialize_file(file_path).map_err(LoadError::SerializationError)?;

        self.file = Some(File {
            file_path: file_path.to_owned(),
            module: Rc::new(module),
            breakpoints: Rc::new(RefCell::new(Breakpoints::new())),
        });
        self.vm = None;

        Ok(())
    }

    pub fn breakpoints(&self) -> DebuggerResult<Ref<'_, HashMap<u32, CodePosition>>> {
        Ok(Ref::map(self.get_file()?.breakpoints.borrow(), |b| {
            &b.breakpoint_indices
        }))
    }

    pub fn add_breakpoint(&mut self, breakpoint: CodePosition) -> DebuggerResult<()> {
        let file = self.get_file_mut()?;
        if let Some(func) = file.module().code_section().and_then(|c| c.bodies().get(breakpoint.func_index)) {
            if func.code().elements().get(breakpoint.instr_index).is_none() {
                return Err(DebuggerError::InvalidBreakpointPosition);
            }
        }
        else {
            return Err(DebuggerError::InvalidBreakpointPosition);
        }
        file.breakpoints
            .borrow_mut()
            .add_breakpoint(breakpoint);
        Ok(())
    }

    pub fn delete_breakpoint(&mut self, index: u32) -> DebuggerResult<bool> {
        Ok(self
            .get_file()?
            .breakpoints
            .borrow_mut()
            .delete_breakpoint(index))
    }

    pub fn run(&mut self) -> DebuggerResult<Trap> {
        Ok(self.create_vm()?.run())
    }

    pub fn call(&mut self, index: u32, args: &[Value]) -> DebuggerResult<Trap> {
        Ok(self.ensure_vm()?.run_func_args(index, args))
    }

    pub fn reset_vm(&mut self) -> DebuggerResult<()> {
        self.create_vm()?;
        Ok(())
    }

    pub fn continue_execution(&mut self) -> DebuggerResult<Trap> {
        if let Some(ref mut vm) = self.vm {
            Ok(vm.continue_execution())
        } else {
            Err(DebuggerError::NoRunningInstance)
        }
    }

    pub fn single_instruction(&mut self) -> DebuggerResult<Option<Trap>> {
        if let Some(ref mut vm) = self.vm {
            Ok(vm.execute_step().err())
        } else {
            Err(DebuggerError::NoRunningInstance)
        }
    }

    pub fn next_instruction(&mut self) -> DebuggerResult<Option<Trap>> {
        if let Some(ref mut vm) = self.vm {
            // Ok(vm.execute_step_over().err())
            Err(DebuggerError::Unimplemented)
        } else {
            Err(DebuggerError::NoRunningInstance)
        }
    }

    fn create_vm(&mut self) -> DebuggerResult<&mut VM> {
        let file = self.file.as_ref().ok_or(DebuggerError::NoFileLoaded)?;
        let module = file.module.clone();
        let breakpoints = file.breakpoints.clone();
        self.vm = Some(VM::new(module, breakpoints).map_err(DebuggerError::InitError)?);
        Ok(self.vm.as_mut().unwrap())
    }

    fn ensure_vm(&mut self) -> DebuggerResult<&mut VM> {
        if let Some(ref mut vm) = self.vm {
            Ok(vm)
        } else {
            self.create_vm()
        }
    }

    fn get_file(&self) -> DebuggerResult<&File> {
        if let Some(ref file) = self.file {
            Ok(file)
        } else {
            Err(DebuggerError::NoFileLoaded)
        }
    }

    fn get_file_mut(&mut self) -> DebuggerResult<&mut File> {
        if let Some(ref mut file) = self.file {
            Ok(file)
        } else {
            Err(DebuggerError::NoFileLoaded)
        }
    }
}
