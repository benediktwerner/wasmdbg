extern crate parity_wasm;

use parity_wasm::{elements::Module, SerializationError};
use std::path::Path;

mod vm;
use vm::VM;


pub enum LoadError {
    FileNotFound,
    SerializationError(SerializationError),
}

pub struct File {
    file_path: String,
    module: Module,
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

    pub fn vm(&self) -> Option<&VM> {
        self.vm.as_ref()
    }

    pub fn load_file(&mut self, file_path: &str) -> Result<(), LoadError> {
        if !Path::new(file_path).exists() {
            return Err(LoadError::FileNotFound);
        }

        self.file = Some(File {
            file_path: file_path.to_owned(),
            module: parity_wasm::deserialize_file(file_path)
                .map_err(LoadError::SerializationError)?,
        });
        self.vm = None;

        Ok(())
    }
}
