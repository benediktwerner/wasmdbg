use std::convert::TryFrom;

use bwasm::ValueType;
use wasmdbg::value::Integer;
use wasmdbg::Debugger;

use super::context;
use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new_subcommand("set")
            .description("Modify various values of the currently running program")
            .requires_running()
            .add_subcommand(
                Command::new("memory", cmd_set_memory)
                    .takes_args("ADDR:addr = VAL:str i8|i16|i32|i64|f32|f64")
                    .description("Modify the linear memory")
                    .help(
                        "Write the the value VAL to the address ADDR in the default linear memory.",
                    ),
            )
            .add_subcommand(
                Command::new("stack", cmd_set_stack)
                    .takes_args("INDEX:usize = VAL:str")
                    .description("Modify a value on the stack")
                    .help(
                        "Replace the value at index INDEX on the stack. The type of the value will be unchanged to preserve wasm validation guarantees.",
                    ),
            )
            .add_subcommand(
                Command::new("local", cmd_set_local)
                    .takes_args("INDEX:usize = VAL:str")
                    .description("Modify the value of a local")
                    .help(
                        "Replace the value of the local with index INDEX.",
                    ),
            )
            .add_subcommand(
                Command::new("stack", cmd_set_global)
                    .takes_args("INDEX:usize = VAL:str")
                    .description("Modify the value of a global")
                    .help(
                        "Replace the value of the global with index INDEX.",
                    ),
            ),
    );
}

enum ValType {
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

impl TryFrom<&str> for ValType {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(match s {
            "i8" => ValType::I8,
            "i16" => ValType::I16,
            "i32" => ValType::I32,
            "i64" => ValType::I64,
            "f32" => ValType::F32,
            "f64" => ValType::F64,
            _ => return Err(format_err!("Invalid type: {}", s)),
        })
    }
}

// TODO: Allow val to be a string e.g. set memory 0x20 = "abc"
fn cmd_set_memory(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let addr = args[0].as_u32();
    let val = args[2].as_string();
    let val = val.as_str();
    let val_type = ValType::try_from(args[3].as_string().as_str())?;

    let memory = dbg.get_vm_mut()?.default_memory_mut()?;

    match val_type {
        ValType::I8 => memory.store(addr, i16::from_str_with_radix(val)? as u8)?,
        ValType::I16 => memory.store(addr, i32::from_str_with_radix(val)? as u16)?,
        ValType::I32 => memory.store(addr, i64::from_str_with_radix(val)? as u32)?,
        ValType::I64 => memory.store(addr, i128::from_str_with_radix(val)? as u64)?,
        ValType::F32 => memory.store(addr, val.parse::<f32>()?)?,
        ValType::F64 => memory.store(addr, val.parse::<f64>()?)?,
    }

    Ok(())
}

fn cmd_set_stack(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let index = args[0].as_usize();
    let val = args[2].as_string();
    let stack = dbg.get_vm_mut()?.value_stack_mut();

    ensure!(index < stack.len(), "Index out of range");

    match stack[index].value_type() {
        ValueType::I32 => stack[index] = (i64::from_str_with_radix(&val)? as u32).into(),
        ValueType::I64 => stack[index] = (i128::from_str_with_radix(&val)? as u64).into(),
        ValueType::F32 => stack[index] = val.parse::<f32>()?.into(),
        ValueType::F64 => stack[index] = val.parse::<f64>()?.into(),
    }

    context::print_context(dbg)
}

fn cmd_set_local(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let index = args[0].as_usize();
    let val = args[2].as_string();
    let locals = dbg.get_vm_mut()?.locals_mut()?;

    ensure!(index < locals.len(), "Index out of range");

    match locals[index].value_type() {
        ValueType::I32 => locals[index] = (i64::from_str_with_radix(&val)? as u32).into(),
        ValueType::I64 => locals[index] = (i128::from_str_with_radix(&val)? as u64).into(),
        ValueType::F32 => locals[index] = val.parse::<f32>()?.into(),
        ValueType::F64 => locals[index] = val.parse::<f64>()?.into(),
    }

    context::print_context(dbg)
}

fn cmd_set_global(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let index = args[0].as_usize();
    let val = args[2].as_string();
    let globals = dbg.get_vm_mut()?.globals_mut();

    ensure!(index < globals.len(), "Index out of range");

    match globals[index].value_type() {
        ValueType::I32 => globals[index] = (i64::from_str_with_radix(&val)? as u32).into(),
        ValueType::I64 => globals[index] = (i128::from_str_with_radix(&val)? as u64).into(),
        ValueType::F32 => globals[index] = val.parse::<f32>()?.into(),
        ValueType::F64 => globals[index] = val.parse::<f64>()?.into(),
    }

    Ok(())
}
