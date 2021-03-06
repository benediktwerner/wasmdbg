use std::cell::Ref;
use std::rc::Rc;

use bwasm::{LoadError, Module};
use thiserror::Error;

use crate::vm::{CodePosition, InitError, Memory, Trap, ImportHandler, VM};
use crate::{Breakpoint, Breakpoints, File, Value};

#[derive(Error, Clone, Debug)]
pub enum DebuggerError {
    #[error("Failed to initialize wasm instance: {0}")]
    InitError(#[from] InitError),
    #[error("No binary file loaded")]
    NoFileLoaded,
    #[error("The binary is not being run")]
    NoRunningInstance,
    #[error("No memory present")]
    NoMemory,
    #[error("Invalid brekapoint position")]
    InvalidBreakpointPosition,
    #[error("Invalid global for watchpoint")]
    InvalidWatchpointGlobal,
    #[error("This feature is still unimplemented")]
    Unimplemented,
}

pub type DebuggerResult<T> = Result<T, DebuggerError>;

#[derive(Default)]
pub struct Debugger {
    file: Option<File>,
    vm: Option<VM>,
}

impl Debugger {
    pub const fn new() -> Self {
        Debugger { file: None, vm: None }
    }

    pub fn file(&self) -> Option<&File> {
        self.file.as_ref()
    }

    pub fn vm(&self) -> Option<&VM> {
        self.vm.as_ref()
    }

    pub fn load_file(&mut self, file_path: &str) -> Result<(), LoadError> {
        let module = Module::from_file(file_path)?;

        self.file = Some(File::new(file_path.to_owned(), module));
        self.vm = None;

        Ok(())
    }

    pub fn backtrace(&self) -> DebuggerResult<Vec<CodePosition>> {
        let vm = self.get_vm()?;
        let mut backtrace = vec![vm.ip()];
        for frame in vm.function_stack().iter().skip(1).rev() {
            backtrace.push(frame.ret_addr);
        }
        Ok(backtrace)
    }

    pub fn globals(&self) -> DebuggerResult<&[Value]> {
        Ok(self.get_vm()?.globals())
    }

    pub fn memory(&self) -> DebuggerResult<&Memory> {
        match self.get_vm()?.default_memory() {
            Ok(memory) => Ok(memory),
            Err(Trap::NoMemory) => Err(DebuggerError::NoMemory),
            Err(_) => unreachable!(),
        }
    }

    pub fn breakpoints(&self) -> DebuggerResult<Ref<'_, Breakpoints>> {
        Ok(self.get_file()?.breakpoints().borrow())
    }

    pub fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> DebuggerResult<u32> {
        let file = self.get_file_mut()?;
        match breakpoint {
            Breakpoint::Code(pos) => {
                if file
                    .module()
                    .get_func(pos.func_index)
                    .and_then(|func| func.instructions().get(pos.instr_index as usize))
                    .is_none()
                {
                    return Err(DebuggerError::InvalidBreakpointPosition);
                }
            }
            Breakpoint::Memory(..) => (),
            Breakpoint::Global(_, index) => {
                if index as usize >= file.module().globals().len() {
                    return Err(DebuggerError::InvalidWatchpointGlobal);
                }
            }
        }
        Ok(file.breakpoints().borrow_mut().add_breakpoint(breakpoint))
    }

    pub fn delete_breakpoint(&mut self, index: u32) -> DebuggerResult<bool> {
        Ok(self.get_file()?.breakpoints().borrow_mut().delete_breakpoint(index))
    }

    pub fn clear_breakpoints(&mut self) -> DebuggerResult<()> {
        self.get_file()?.breakpoints().borrow_mut().clear();
        Ok(())
    }

    pub fn run(&mut self) -> DebuggerResult<Trap> {
        Ok(self.create_vm()?.run())
    }

    pub fn start(&mut self) -> DebuggerResult<Option<Trap>> {
        Ok(self.create_vm()?.start().err())
    }

    pub fn call(&mut self, index: u32, args: &[Value]) -> DebuggerResult<Trap> {
        Ok(self.ensure_vm()?.run_func(index, args))
    }

    pub fn reset_vm(&mut self) -> DebuggerResult<()> {
        self.vm = None;
        Ok(())
    }

    pub fn continue_execution(&mut self) -> DebuggerResult<Trap> {
        Ok(self.get_vm_mut()?.continue_execution())
    }

    pub fn execute_step(&mut self) -> DebuggerResult<Option<Trap>> {
        Ok(self.get_vm_mut()?.execute_step().err())
    }

    pub fn execute_step_over(&mut self) -> DebuggerResult<Option<Trap>> {
        Ok(self.get_vm_mut()?.execute_step_over().err())
    }

    pub fn execute_step_out(&mut self) -> DebuggerResult<Option<Trap>> {
        Ok(self.get_vm_mut()?.execute_step_out().err())
    }

    fn create_vm(&mut self) -> DebuggerResult<&mut VM> {
        let file = self.file.as_ref().ok_or(DebuggerError::NoFileLoaded)?;
        let module = Rc::clone(file.module());
        let breakpoints = Rc::clone(file.breakpoints());
        self.vm = Some(VM::new(module, breakpoints, ImportHandler::default()).map_err(DebuggerError::InitError)?);
        Ok(self.vm.as_mut().unwrap())
    }

    fn ensure_vm(&mut self) -> DebuggerResult<&mut VM> {
        if let Some(ref mut vm) = self.vm {
            Ok(vm)
        } else {
            self.create_vm()
        }
    }

    pub fn get_vm(&self) -> DebuggerResult<&VM> {
        if let Some(ref vm) = self.vm {
            Ok(vm)
        } else {
            Err(DebuggerError::NoRunningInstance)
        }
    }

    pub fn get_vm_mut(&mut self) -> DebuggerResult<&mut VM> {
        if let Some(ref mut vm) = self.vm {
            Ok(vm)
        } else {
            Err(DebuggerError::NoRunningInstance)
        }
    }

    pub fn get_file(&self) -> DebuggerResult<&File> {
        if let Some(ref file) = self.file {
            Ok(file)
        } else {
            Err(DebuggerError::NoFileLoaded)
        }
    }

    pub fn get_file_mut(&mut self) -> DebuggerResult<&mut File> {
        if let Some(ref mut file) = self.file {
            Ok(file)
        } else {
            Err(DebuggerError::NoFileLoaded)
        }
    }
}
