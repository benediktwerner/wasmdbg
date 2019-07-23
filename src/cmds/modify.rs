use std::convert::TryFrom;

use failure::Error;

use wasmdbg::value::Integer;
use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new_subcommand("set")
            .description("Modify various values of the currently running program")
            // .requires_running()
            .add_subcommand(
                Command::new("memory", cmd_set_memory)
                    .takes_args("ADDR:addr = VAL:str i8|i16|i32|i64|f32|f64")
                    .description("Modify the linear memory")
                    .help(
                        "Write the the value VAL to the address ADDR in the default linear memory.",
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
    type Error = Error;
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

    let memory = dbg.get_vm_mut()?.memory_mut();

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
