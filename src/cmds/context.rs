use colored::*;
use parity_wasm::elements::Instruction;

use wasmdbg::vm::{CodePosition, ModuleHelper};
use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};
use crate::utils::{print_header, print_line};

const DISASSEMBLY_DEFAULT_MAX_LINES: usize = 20;

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("locals", cmd_locals)
            .takes_args("[all|COUNT:usize]")
            .description("Print locals")
            .help("Print the values of the locals of the current function"),
    );
    commands.add(
        Command::new("disassemble", cmd_disassemble)
            .alias("disas")
            .alias("disass")
            .takes_args("[FUNC_INDEX:u32]")
            .description("Disassemble code")
            .help("Disassemble the current function or the one with the specified index."),
    );
    commands.add(Command::new("stack", cmd_stack).description("Print the current value stack"));
    commands.add(
        Command::new("labels", cmd_labels)
            .takes_args("[all|COUNT:usize]")
            .description("Print the current label stack"),
    );
    commands.add(
        Command::new("backtrace", cmd_backtrace)
            .takes_args("[all|COUNT:usize]")
            .description("Print a function backtrace"),
    );
    commands
        .add(Command::new("context", cmd_context).description("Show current execution context"));

    commands.add(
        Command::new("globals", cmd_globals)
            .description("Print globals")
            .description("Print the values of the globals"),
    );
}

fn cmd_locals(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let max_count = match args.get(0) {
        Some(CmdArg::Const("all")) => usize::max_value(),
        Some(CmdArg::Usize(count)) => *count,
        None => 17,
        _ => unreachable!(),
    };
    let locals = dbg.locals()?;
    if locals.is_empty() {
        println!("<no locals>");
    } else {
        let locals_trimmed = if locals.len() > max_count {
            &locals[..max_count]
        } else {
            locals
        };
        let max_index_len = locals_trimmed.len().to_string().len();
        for (i, local) in locals_trimmed.iter().enumerate() {
            println!("Local {:>2$}: {}", i, local, max_index_len);
        }
        if locals.len() > max_count {
            println!("...");
        }
    }
    Ok(())
}

fn cmd_disassemble(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let index = match args.get(0) {
        Some(CmdArg::U32(func_index)) => *func_index,
        None => dbg.get_vm()?.ip().func_index,
        _ => unreachable!(),
    };
    if let Some(code) = dbg
        .get_file()?
        .module()
        .get_func(index)
        .map(|b| b.code().elements())
    {
        if args.is_empty() && code.len() > DISASSEMBLY_DEFAULT_MAX_LINES {
            let ip = dbg.get_vm()?.ip();
            let start = if ip.instr_index as usize > code.len() - DISASSEMBLY_DEFAULT_MAX_LINES {
                code.len() - DISASSEMBLY_DEFAULT_MAX_LINES
            } else {
                ip.instr_index.max(2) as usize - 2
            };
            let end = start + DISASSEMBLY_DEFAULT_MAX_LINES;
            print_disassembly(
                dbg,
                CodePosition::new(index, start as u32),
                &code[start..end],
            );
        } else {
            print_disassembly(dbg, CodePosition::new(index, 0), code);
        }
    } else {
        bail!("Invalid function index");
    }
    Ok(())
}

fn cmd_stack(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let value_stack = dbg.get_vm()?.value_stack();
    if value_stack.is_empty() {
        println!("<empty>");
        return Ok(());
    }
    let max_index_len = value_stack.len().to_string().len();
    for (i, value) in value_stack.iter().enumerate().rev() {
        println!(" {:>2$}: {}", i, value, max_index_len);
    }
    Ok(())
}

fn cmd_labels(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let mut max_count = match args.get(0) {
        Some(CmdArg::Const("all")) => usize::max_value(),
        Some(CmdArg::Usize(count)) => *count,
        None => 5,
        _ => unreachable!(),
    };
    let labels = dbg.get_vm()?.label_stack();
    if labels.len() < max_count {
        max_count = labels.len();
    }
    for (i, label) in labels[labels.len() - max_count..].iter().rev().enumerate() {
        // TODO: Print labels properly
        println!("{:>3}: {:?}", labels.len() - i - 1, label);
    }
    Ok(())
}

fn cmd_backtrace(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let mut max_count = match args.get(0) {
        Some(CmdArg::Const("all")) => usize::max_value(),
        Some(CmdArg::Usize(count)) => *count,
        None => 5,
        _ => unreachable!(),
    };
    let backtrace = dbg.backtrace()?;
    if backtrace.len() < max_count {
        max_count = backtrace.len();
    }
    if let Some(curr_func) = backtrace.first() {
        println!("=> f {:<10}{}", curr_func.func_index, curr_func.instr_index);
        for func in &backtrace[1..max_count] {
            println!("   f {:<10}{}", func.func_index, func.instr_index);
        }
    } else {
        println!("WTF? No function backtrace...");
    }
    Ok(())
}

fn cmd_context(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    print_context(dbg)
}

fn print_disassembly(dbg: &Debugger, start: CodePosition, instrs: &[Instruction]) {
    let curr_instr_index = dbg.vm().and_then(|vm| {
        if vm.ip().func_index == start.func_index {
            Some(vm.ip().instr_index)
        } else {
            None
        }
    });
    let max_index_len = (start.instr_index as usize + instrs.len() - 1)
        .to_string()
        .len();
    let breakpoints = dbg.breakpoints().ok();
    for (i, instr) in instrs.iter().enumerate() {
        let instr_index = start.instr_index + i as u32;
        let addr_str = format!("{}:{:>02$}", start.func_index, instr_index, max_index_len);
        let breakpoint = match breakpoints {
            Some(ref breakpoints) => {
                breakpoints.find(CodePosition::new(start.func_index, instr_index))
            }
            None => None,
        };
        let breakpoint_str = match breakpoint {
            Some(_) => "*".red().to_string(),
            None => " ".to_string(),
        };
        if curr_instr_index.map_or(false, |i| i == instr_index) {
            println!("=> {}{}   {}", breakpoint_str, addr_str.green(), instr);
        } else {
            println!("   {}{}   {}", breakpoint_str, addr_str, instr);
        }
    }
}

pub fn print_context(dbg: &mut Debugger) -> CmdResult {
    print_header("LOCALS");
    cmd_locals(dbg, &[])?;
    print_header("DISASM");
    cmd_disassemble(dbg, &[])?;
    print_header("VALUE STACK");
    cmd_stack(dbg, &[])?;
    print_header("LABEL STACK");
    cmd_labels(dbg, &[])?;
    print_header("BACKTRACE");
    cmd_backtrace(dbg, &[])?;
    print_line();
    Ok(())
}

fn cmd_globals(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let globals = dbg.globals()?;
    if globals.is_empty() {
        println!("<no locals>");
    } else {
        let max_index_len = globals.len().to_string().len();
        for (i, global) in globals.iter().enumerate() {
            println!("Global {:>2$}: {}", i, global, max_index_len);
        }
    }
    Ok(())
}
