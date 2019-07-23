use wasmdbg::Debugger;

use super::format::{fmt_char, Format};
use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("x", cmd_x)
            .takes_args("/FMT ADDRESS:addr")
            .description("Examine memory")
            .requires_running(),
    );
}

fn cmd_x(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let ((count, size, format), address) = if let CmdArg::Fmt(count, size, format) = &args[0] {
        ((*count, *size, *format), args[1].as_u32())
    } else {
        ((1, 4, Format::Hex), args[0].as_u32())
    };
    let memory = dbg.memory()?;
    let mut addr = address;
    for _ in 0..count {
        if let Format::Str = format {
            let bytes: Vec<u8> = memory.data()[addr as usize..]
                .iter()
                .cloned()
                .take_while(|b| *b != 0)
                .collect();
            let val_str: String = bytes.iter().flat_map(|b| fmt_char(b)).collect();
            println!(" 0x{:08x}: \"{}\"", addr, val_str);
            addr += bytes.len() as u32 + 1;
        } else {
            let val_str = match size {
                1 => format.format(memory.load::<u8>(addr)?),
                2 => format.format(memory.load::<u16>(addr)?),
                4 => format.format(memory.load::<u32>(addr)?),
                8 => format.format(memory.load::<u64>(addr)?),
                _ => unreachable!(),
            };
            println!(" 0x{:08x}: {}", addr, val_str);
            addr += count * size;
        }
    }
    Ok(())
}
