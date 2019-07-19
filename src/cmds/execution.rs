use wasmdbg::value::Value;
use wasmdbg::vm::{CodePosition, ModuleHelper, Trap};
use wasmdbg::Debugger;

use super::context;
use super::{CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("run", cmd_run)
            .alias("r")
            .description("Run the currently loaded binary")
            .requires_file(),
    );
    commands.add(
        Command::new("call", cmd_call)
            .description("Call a specific function in the current runtime context")
            .takes_args_range(1..=20)
            .requires_file(),
    );
    commands.add(
            Command::new("break", cmd_break)
                .alias("b")
                .takes_args_range(1..=2)
                .description("Set a breakpoint")
                .help("break FUNC_INDEX [INSTRUCTION_INDEX]\n\nSet a breakpoint at the specified function and instruction. If no instruction is specified the breakpoint is set to the function start. When execution reaches a breakpoint it will pause")
                .requires_file(),
        );
    commands.add(
        Command::new("delete", cmd_delete)
            .description("Delete a breakpoint")
            .help("delete BREAKPOINT_INDEX\n\nDelete the breakpoint with the specified index.")
            .takes_args(1)
            .requires_file(),
    );
    commands.add(
        Command::new("continue", cmd_continue)
            .alias("c")
            .description("Continue execution after a breakpoint")
            .requires_running(),
    );
    commands.add(
        Command::new("step", cmd_step)
            .alias("stepi")
            .alias("s")
            .alias("si")
            .takes_args_range(0..=1)
            .description("Step one instruction")
            .help("step [N]\n\nStep exactly one or if an argument is given exactly N instructions.\nUnlike \"next\" this will enter subroutine calls.")
            .requires_running()
    );
    commands.add(
        Command::new("next", cmd_next)
            .alias("nexti")
            .alias("n")
            .alias("ni")
            .takes_args_range(0..=1)
            .description("Step one instruction, but skip over subroutine calls")
            .help("next [N]\n\nStep one or if an argument is given N instructions.\nUnlike \"step\" this will skip over subroutine calls.")
            .requires_running()
    );
    commands.add(
        Command::new("finish", cmd_finish)
            .description("Execute until the current function returns")
            .requires_running(),
    );
}

fn cmd_run(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    print_run_result(dbg.run()?, dbg)
}

fn cmd_call(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let module = dbg.module().unwrap();
    let func_index = args[0].parse()?;
    let args = &args[1..];

    let func_type = module
        .get_func_type(func_index)
        .ok_or_else(|| format_err!("No function with index {}", func_index))?;

    if args.len() != func_type.params().len() {
        bail!(
            "Invalid number of arguments. Function #{} takes {} args but got {}",
            func_index,
            func_type.params().len(),
            args.len()
        );
    }

    let mut args_parsed = Vec::new();
    for (arg, value_type) in args.iter().zip(func_type.params().iter()) {
        if let Some(arg_parsed) = Value::from_str(arg, *value_type) {
            args_parsed.push(arg_parsed);
        } else {
            bail!("Failed to parse argument \"{}\" as {}", arg, value_type);
        }
    }

    print_run_result(dbg.call(func_index, &args_parsed)?, dbg)
}

fn cmd_break(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let func_index = args[0].parse()?;
    let instr_index = args.get(1).map(|n| n.parse()).transpose()?.unwrap_or(0);
    let breakpoint = CodePosition {
        func_index,
        instr_index,
    };
    let index = dbg.add_breakpoint(breakpoint)?;
    println!("Set breakpoint {} at {}:{}", index, func_index, instr_index);
    Ok(())
}

fn cmd_delete(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let index = args[0].parse()?;
    if dbg.delete_breakpoint(index)? {
        println!("Breakpoint removed");
    } else {
        bail!("No breakpoint with index {}", index);
    }
    Ok(())
}

fn cmd_continue(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    print_run_result(dbg.continue_execution()?, dbg)
}

fn cmd_step(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let steps: u32 = args.get(0).map(|n| n.parse()).transpose()?.unwrap_or(1);
    for _ in 0..steps {
        if let Some(trap) = dbg.single_instruction()? {
            return print_run_result(trap, dbg);
        }
    }
    context::print_context(dbg)
}

fn cmd_next(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let steps: u32 = args.get(0).map(|n| n.parse()).transpose()?.unwrap_or(1);
    for _ in 0..steps {
        if let Some(trap) = dbg.next_instruction()? {
            return print_run_result(trap, dbg);
        }
    }
    context::print_context(dbg)
}

fn cmd_finish(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    if let Some(trap) = dbg.execute_until_return()? {
        print_run_result(trap, dbg)
    } else {
        context::print_context(dbg)
    }
}

fn print_run_result(trap: Trap, dbg: &mut Debugger) -> CmdResult {
    match trap {
        Trap::ExecutionFinished => {
            if let Some(result) = dbg.get_vm()?.value_stack().first() {
                println!("Finished execution => {:?}", result);
            } else {
                println!("Finished execution")
            }
        }
        Trap::BreakpointReached(index) => {
            println!("Reached breakpoint {}", index);
            context::print_context(dbg)?;
        }
        _ => println!("Trap: {}", trap),
    }
    Ok(())
}
