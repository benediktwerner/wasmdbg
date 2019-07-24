use wasmdbg::breakpoints::{Breakpoint, BreakpointTrigger};
use wasmdbg::value::Value;
use wasmdbg::vm::{CodePosition, Trap};
use wasmdbg::Debugger;

use super::context;
use super::{CmdArg, CmdArgOptionExt, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("run", cmd_run)
            .alias("r")
            .description("Run the currently loaded binary")
            .requires_file(),
    );
    commands.add(
        Command::new("start", cmd_start)
            .description("Start the currently loaded binary and pause on the first instruction")
            .requires_file(),
    );
    commands.add(
        Command::new("call", cmd_call)
            .takes_args("FUNC_INDEX:u32 [ARGS:str...]")
            .description("Call a specific function in the current runtime context")
            .requires_file(),
    );
    commands.add(
            Command::new("break", cmd_break)
                .alias("b")
                .takes_args("FUNC_INDEX:u32 [INSTRUCTION_INDEX:u32]")
                .description("Set a breakpoint")
                .help("Set a breakpoint at the specified function and instruction. If no instruction is specified the breakpoint is set to the function start. When execution reaches a breakpoint it will pause.")
            .requires_file()
        );
    commands.add(
            Command::new_subcommand("watch")
            .requires_file()
            .add_subcommand(Command::new("memory", cmd_watch_memory).takes_args("ADDR:addr [read|write]").description("Watch a memory location").help("Watch the memory at address ADDR and pause execution when it's value is read/written."))
            .add_subcommand(Command::new("global", cmd_watch_global).takes_args("INDEX:u32 [read|write]").description("Watch a global").help("Watch the global with index INDEX and pause execution when it's value is read/written."))
        );
    commands.add(
        Command::new("delete", cmd_delete)
            .description("Delete a breakpoint")
            .takes_args("BREAKPOINT_INDEX:u32")
            .help("Delete the breakpoint with the specified index.")
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
            .takes_args("[N:u32]")
            .description("Step one instruction")
            .help("Step exactly one or if an argument is given exactly N instructions.\nUnlike \"next\" this will enter subroutine calls.")
            .requires_running()
    );
    commands.add(
        Command::new("next", cmd_next)
            .alias("nexti")
            .alias("n")
            .alias("ni")
            .takes_args("[N:u32]")
            .description("Step one instruction, but skip over subroutine calls")
            .help("Step one or if an argument is given N instructions.\nUnlike \"step\" this will skip over subroutine calls.")
            .requires_running()
    );
    commands.add(
        Command::new("finish", cmd_finish)
            .description("Execute until the current function returns")
            .requires_running(),
    );
}

fn cmd_run(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    print_run_result(dbg.run()?, dbg)
}

fn cmd_start(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    if let Some(trap) = dbg.start()? {
        print_run_result(trap, dbg)
    } else {
        context::print_context(dbg)
    }
}

fn cmd_call(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let module = dbg.get_file()?.module();
    let func_index = args[0].as_u32();
    let args = &args[1..];

    let func_type = module
        .get_func(func_index)
        .ok_or_else(|| format_err!("No function with index {}", func_index))?
        .func_type();

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
        if let Some(arg_parsed) = Value::from_str(&arg.as_string(), *value_type) {
            args_parsed.push(arg_parsed);
        } else {
            bail!(
                "Failed to parse argument \"{}\" as {}",
                arg.as_string(),
                value_type
            );
        }
    }

    print_run_result(dbg.call(func_index, &args_parsed)?, dbg)
}

fn cmd_break(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let func_index = args[0].as_u32();
    let instr_index = args.get(1).as_u32_or(0);
    let pos = CodePosition {
        func_index,
        instr_index,
    };
    let index = dbg.add_breakpoint(Breakpoint::Code(pos))?;
    println!("Set breakpoint {} at {}", index, pos);
    Ok(())
}

fn cmd_watch_memory(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let addr = args[0].as_u32();
    let trigger = match args.get(1) {
        Some(trigger) => match trigger.as_const() {
            "read" => BreakpointTrigger::Read,
            "write" => BreakpointTrigger::Write,
            trigger => bail!("Invalid watchpoint trigger: {}", trigger),
        },
        None => BreakpointTrigger::ReadWrite,
    };
    let index = dbg.add_breakpoint(Breakpoint::Memory(trigger, addr))?;
    println!("Set watchpoint {} at address 0x{:>08x}", index, addr);
    Ok(())
}

fn cmd_watch_global(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let index = args[0].as_u32();
    let trigger = match args.get(1) {
        Some(trigger) => match trigger.as_const() {
            "read" => BreakpointTrigger::Read,
            "write" => BreakpointTrigger::Write,
            trigger => bail!("Invalid watchpoint trigger: {}", trigger),
        },
        None => BreakpointTrigger::ReadWrite,
    };
    let index = dbg.add_breakpoint(Breakpoint::Global(trigger, index))?;
    println!("Set watchpoint {} at global {}", index, index);
    Ok(())
}

fn cmd_delete(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let index = args[0].as_u32();
    if dbg.delete_breakpoint(index)? {
        println!("Breakpoint removed");
    } else {
        bail!("No breakpoint with index {}", index);
    }
    Ok(())
}

fn cmd_continue(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    print_run_result(dbg.continue_execution()?, dbg)
}

fn cmd_step(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let steps = args.get(0).as_u32_or(1);
    for _ in 0..steps {
        if let Some(trap) = dbg.single_instruction()? {
            return print_run_result(trap, dbg);
        }
    }
    context::print_context(dbg)
}

fn cmd_next(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let steps = args.get(0).as_u32_or(1);
    for _ in 0..steps {
        if let Some(trap) = dbg.next_instruction()? {
            return print_run_result(trap, dbg);
        }
    }
    context::print_context(dbg)
}

fn cmd_finish(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
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
                println!("Finished execution => {}", result);
            } else {
                println!("Finished execution")
            }
        }
        Trap::BreakpointReached(index) => {
            context::print_context(dbg)?;
            println!("Reached breakpoint {}", index);
        }
        Trap::WatchpointReached(index) => {
            context::print_context(dbg)?;
            println!("Reached watchpoint {}", index);
        }
        _ => println!("Trap: {}", trap),
    }
    Ok(())
}
