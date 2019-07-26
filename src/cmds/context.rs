use colored::*;

use wasmdbg::vm::CodePosition;
use wasmdbg::wasm::Instruction;
use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};
use crate::utils::{print_header, print_line};

const DISASSEMBLY_DEFAULT_MAX_LINES: u32 = 18;

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("locals", cmd_locals)
            .takes_args("[all|COUNT:usize]")
            .description("Print locals")
            .help("Print the values of the locals of the current function")
            .requires_running(),
    );
    commands.add(
        Command::new("nearpc", cmd_nearpc)
            .takes_args("[FORWARDS:u32 [BACKWARDS:u32]]")
            .description("Disassemble around current instruction")
            .requires_running(),
    );
    commands.add(
        Command::new("disassemble", cmd_disassemble)
            .alias("disas")
            .alias("disass")
            .takes_args("[FUNC_INDEX:u32]")
            .description("Disassemble code")
            .help("Disassemble the current function or the one with the specified index.")
            .requires_file(),
    );
    commands.add(Command::new("stack", cmd_stack).description("Print the current value stack"));
    commands.add(
        Command::new("labels", cmd_labels)
            .takes_args("[all|COUNT:usize]")
            .description("Print the current label stack")
            .requires_running(),
    );
    commands.add(
        Command::new("backtrace", cmd_backtrace)
            .takes_args("[all|COUNT:usize]")
            .description("Print a function backtrace")
            .requires_running(),
    );
    commands.add(
        Command::new("context", cmd_context)
            .description("Show current execution context")
            .requires_running(),
    );

    commands.add(
        Command::new("globals", cmd_globals)
            .description("Print globals")
            .description("Print the values of the globals")
            .requires_running(),
    );
}

fn cmd_locals(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let max_count = match args.get(0) {
        Some(CmdArg::Const("all")) => usize::max_value(),
        Some(CmdArg::Usize(count)) => *count,
        None => 17,
        _ => unreachable!(),
    };
    let locals = dbg.get_vm()?.locals()?;
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

fn cmd_nearpc(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let (forward, back) = match args.get(0) {
        Some(count) => {
            let count = count.as_u32();
            match args.get(1) {
                Some(back) => (count, back.as_u32()),
                None => (count, 2),
            }
        }
        None => (DISASSEMBLY_DEFAULT_MAX_LINES, 2),
    };
    let ip = dbg.get_vm()?.ip();
    let code = dbg
        .get_file()?
        .module()
        .get_func(ip.func_index)
        .unwrap()
        .instructions();
    if forward + back >= code.len() as u32 {
        print_disassembly(dbg, CodePosition::new(ip.func_index, 0), None)
    } else {
        let start = ip.instr_index - back.min(ip.instr_index);
        let end = (ip.instr_index + forward).min(code.len() as u32);
        print_disassembly(
            dbg,
            CodePosition::new(ip.func_index, start),
            Some(end - start),
        )
    }
}

fn cmd_disassemble(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    match args.get(0) {
        Some(func_index) => print_disassembly(dbg, CodePosition::new(func_index.as_u32(), 0), None),

        None => cmd_nearpc(dbg, &[]),
    }
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

fn cmd_context(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    print_context(dbg)
}

fn print_disassembly(dbg: &Debugger, start: CodePosition, len: Option<u32>) -> CmdResult {
    let curr_instr_index = dbg.vm().and_then(|vm| {
        if vm.ip().func_index == start.func_index {
            Some(vm.ip().instr_index)
        } else {
            None
        }
    });
    let code = match dbg.get_file()?.module().get_func(start.func_index) {
        Some(func) => {
            ensure!(
                !func.is_imported(),
                "Cannot show disassembly of imported function"
            );
            let start = start.instr_index as usize;
            if let Some(len) = len {
                let end = start + len as usize;
                &func.instructions()[start..end]
            } else {
                &func.instructions()[start..]
            }
        }
        None => bail!("Invalid instruction index: {}", start.func_index),
    };
    let max_index_len = (start.instr_index as usize + code.len()).to_string().len();
    let breakpoints = dbg.breakpoints().ok();
    let mut indent = calc_start_indent(code);
    for (i, instr) in code.iter().enumerate() {
        let instr_index = start.instr_index + i as u32;
        let addr_str = format!("{}:{:>02$}", start.func_index, instr_index, max_index_len);
        let breakpoint = match breakpoints {
            Some(ref breakpoints) => {
                breakpoints.find_code(CodePosition::new(start.func_index, instr_index))
            }
            None => None,
        };
        let breakpoint_str = match breakpoint {
            Some(_) => "*".red().to_string(),
            None => " ".to_string(),
        };
        let instr_str = format_instr(dbg, instr)?;
        match instr {
            Instruction::Else => indent -= 1,
            Instruction::End => indent -= 1,
            _ => (),
        }
        if curr_instr_index.map_or(false, |i| i == instr_index) {
            // TODO: if instr is call: print args
            println!(
                "=> {}{}   {: >4$}{}",
                breakpoint_str,
                addr_str.green(),
                "",
                instr_str,
                indent
            );
        } else {
            println!(
                "   {}{}   {: >4$}{}",
                breakpoint_str, addr_str, "", instr_str, indent
            );
        }
        match instr {
            Instruction::Block(_) => indent += 1,
            Instruction::Loop(_) => indent += 1,
            Instruction::If(_) => indent += 1,
            Instruction::Else => indent += 1,
            _ => (),
        }
    }
    Ok(())
}

fn format_instr(dbg: &Debugger, instr: &Instruction) -> Result<String, failure::Error> {
    let result = match instr {
        Instruction::Call(index) => format!(
            "{} <{}>",
            instr,
            dbg.get_file()?.module().get_func(*index).unwrap().name()
        ),
        _ => instr.to_string(),
    };
    Ok(result)
}

fn calc_start_indent(code: &[Instruction]) -> usize {
    let mut indent: isize = 0;
    let mut min_indent: isize = 0;
    for instr in code {
        match instr {
            Instruction::Block(_) => indent += 1,
            Instruction::Loop(_) => indent += 1,
            Instruction::If(_) => indent += 1,
            Instruction::Else => {
                if indent == min_indent {
                    min_indent -= 1;
                }
            }
            Instruction::End => {
                indent -= 1;
                if indent < min_indent {
                    min_indent = indent;
                }
            }
            _ => (),
        }
    }
    if min_indent < 0 {
        (-min_indent) as usize
    } else {
        0
    }
}

pub fn print_context(dbg: &mut Debugger) -> CmdResult {
    print_header("LOCALS");
    cmd_locals(dbg, &[])?;
    print_header("DISASM");
    cmd_nearpc(dbg, &[])?;
    print_header("VALUE STACK");
    cmd_stack(dbg, &[])?;
    print_header("LABEL STACK");
    cmd_labels(dbg, &[])?;
    print_header("BACKTRACE");
    cmd_backtrace(dbg, &[])?;
    print_line();
    Ok(())
}
