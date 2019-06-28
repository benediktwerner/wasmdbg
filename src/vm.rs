extern crate parity_wasm;

use parity_wasm::{elements::Module, SerializationError};


enum Value {
    I32(u32),
    I64(u64),
    F32(f32),
    F64(f64),
}

struct CodePosition(usize, usize);

enum Control {
    Label(usize),
    Return(CodePosition),
}

struct Memory {
    data: Vec<u8>,
}

pub struct VM {
    module: Box<Module>,
    memory: Memory,
    ip: CodePosition,
    value_stack: Vec<Value>,
    control_stack: Vec<Control>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory { data: Vec::new() }
    }
}

impl VM {
    pub fn new(module: Box<Module>) -> VM {
        VM {
            memory: Memory::new(),
            module: module,
            ip: CodePosition(0, 0),
            value_stack: Vec::new(),
            control_stack: Vec::new(),
        }
    }

    pub fn execute_step(&self) {
        let CodePosition(func_index, instr_index) = self.ip;
        let func = &self.module.code_section().unwrap().bodies()[func_index];
        let instr = &func.code().elements()[instr_index];

        match instr {
            _ => println!("Unknown instruction: {}", instr),
        }
    }
}
