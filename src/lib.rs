extern crate parity_wasm;

use parity_wasm::{elements::Module, SerializationError};
use std::fmt;
use std::path::Path;
use std::rc::Rc;

pub mod nan_preserving_float;
pub mod value;
pub mod vm;
use value::Value;
use vm::{InitError, Trap, VM};


pub enum LoadError {
    FileNotFound,
    SerializationError(SerializationError),
}

pub enum DebuggerError {
    InitError(InitError),
    NoFileLoaded,
}

impl fmt::Display for DebuggerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DebuggerError::InitError(error) => {
                write!(f, "Failed to initialize wasm instance: {}", error)
            }
            DebuggerError::NoFileLoaded => write!(f, "No binary file loaded"),
        }
    }
}

pub type DebuggerResult<T> = Result<T, DebuggerError>;

pub struct File {
    file_path: String,
    module: Rc<Module>,
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
        });
        self.vm = None;

        Ok(())
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

    fn create_vm(&mut self) -> DebuggerResult<&mut VM> {
        let file = self.file.as_ref().ok_or(DebuggerError::NoFileLoaded)?;
        let module = file.module.clone();
        self.vm = Some(VM::new(module).map_err(DebuggerError::InitError)?);
        Ok(self.vm.as_mut().unwrap())
    }

    fn ensure_vm(&mut self) -> DebuggerResult<&mut VM> {
        if let Some(ref mut vm) = self.vm {
            Ok(vm)
        } else {
            self.create_vm()
        }
    }
}
