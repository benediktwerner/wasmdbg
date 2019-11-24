use std::cell::RefCell;
use std::rc::Rc;

use bwasm::Module;

use crate::Breakpoints;

pub struct File {
    file_path: String,
    module: Rc<Module>,
    breakpoints: Rc<RefCell<Breakpoints>>,
}

impl File {
    pub fn new(file_path: String, module: Module) -> Self {
        File {
            file_path,
            module: Rc::new(module),
            breakpoints: Rc::new(RefCell::new(Breakpoints::new())),
        }
    }

    pub const fn file_path(&self) -> &String {
        &self.file_path
    }

    pub const fn module(&self) -> &Rc<Module> {
        &self.module
    }

    pub const fn breakpoints(&self) -> &Rc<RefCell<Breakpoints>> {
        &self.breakpoints
    }
}
