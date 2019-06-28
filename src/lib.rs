extern crate parity_wasm;

use parity_wasm::{elements::Module, SerializationError};

struct File {
    file_path: String,
    module: Module,
}

pub struct Debugger {
    file: Option<File>,
}


impl Debugger {
    pub fn new() -> Debugger {
        Debugger { file: None }
    }

    pub fn load_file(&mut self, file_path: &str) -> Result<(), SerializationError> {
        self.file = Some(File {
            file_path: file_path.to_owned(),
            module: parity_wasm::deserialize_file(file_path)?,
        });
        Ok(())
    }
}
