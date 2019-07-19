use wasmdbg::vm::{CodePosition, Trap};
use wasmdbg::Debugger;

use super::{CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new_subcommand("info")
            .add_subcommand(
                Command::new("file", cmd_info_file)
                    .description("Print info about the currently loaded binary"),
            )
            .add_subcommand(
                Command::new("breakpoints", cmd_info_break)
                    .alias("break")
                    .description("Print info about breakpoints"),
            )
            .add_subcommand(
                Command::new("ip", cmd_info_ip)
                    .description("Print the current instruction pointer")
                    .requires_running(),
            )
            .alias("i")
            .description("Print info about the programm being debugged")
            .requires_file(),
    );
    commands.add(
        Command::new("status", cmd_status)
            .description("Print status of the current wasm instance")
            .requires_running(),
    );
}

fn cmd_info_file(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    let file = dbg.file().unwrap();
    let module = file.module();

    println!("File: {}", file.file_path());

    match module.function_section() {
        Some(func_sec) => println!("{} functions", func_sec.entries().len()),
        None => println!("No functions"),
    }

    Ok(())
}

fn cmd_info_break(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    let breakpoints = dbg.breakpoints()?;
    ensure!(breakpoints.len() > 0, "No breakpoints");

    let mut breakpoints: Vec<(&u32, &CodePosition)> = breakpoints.iter().collect();
    breakpoints.sort_unstable_by(|(index1, _), (index2, _)| index1.cmp(index2));

    println!("{:<8}{:<12}Instruction", "Num", "Function");
    for (index, breakpoint) in breakpoints {
        println!(
            "{:<8}{:<12}{}",
            index, breakpoint.func_index, breakpoint.instr_index
        );
    }

    Ok(())
}

fn cmd_info_ip(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    let ip = dbg.vm().unwrap().ip();
    println!("Function: {}", ip.func_index);
    println!("Instruction: {}", ip.instr_index);
    Ok(())
}

fn cmd_status(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    if let Some(trap) = dbg.vm().unwrap().trap() {
        if let Trap::ExecutionFinished = trap {
            println!("Finished execution");
        } else {
            println!("Trap: {}", trap);
        }
    } else {
        println!("No trap");
        let ip = dbg.vm().unwrap().ip();
        println!("Function: {}", ip.func_index);
        println!("Instruction: {}", ip.instr_index);
    }
    Ok(())
}
