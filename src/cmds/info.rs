use wasmdbg::vm::{CodePosition, Trap};
use wasmdbg::wasm::{External, InitExpr, PAGE_SIZE};
use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new_subcommand("info")
            .alias("i")
            .description("Print info about the programm being debugged")
            .requires_file()
            .add_subcommand(
                Command::new("file", cmd_info_file)
                    .description("Print info about the currently loaded binary"),
            )
            .add_subcommand(
                Command::new("breakpoints", cmd_info_break)
                    .alias("break")
                    .description("Print breakpoints"),
            )
            .add_subcommand(
                Command::new("ip", cmd_info_ip)
                    .description("Print instruction pointer")
                    .requires_running(),
            )
            .add_subcommand(Command::new("types", cmd_info_types).description("Print type section"))
            .add_subcommand(
                Command::new("imports", cmd_info_imports).description("Print import section"),
            )
            .add_subcommand(
                Command::new("functions", cmd_info_functions)
                    .alias("funcs")
                    .description("Print function section"),
            )
            .add_subcommand(Command::new("tables", cmd_info_tables).description("Print tables"))
            .add_subcommand(
                Command::new("memory", cmd_info_memory).description("Print memory section"),
            )
            .add_subcommand(Command::new("globals", cmd_info_globals).description("Print globals"))
            .add_subcommand(Command::new("exports", cmd_info_exports).description("Print exports"))
            .add_subcommand(
                Command::new("start", cmd_info_start).description("Print start section"),
            )
            .add_subcommand(
                Command::new("elements", cmd_info_elements).description("Print element section"),
            )
            .add_subcommand(Command::new("data", cmd_info_data).description("Print data section"))
            .add_subcommand(
                Command::new("custom", cmd_info_custom)
                    .takes_args("[INDEX:u32|NAME:str]")
                    .description("Print custom sections"),
            ),
    );
    commands.add(
        Command::new("status", cmd_status)
            .description("Print status of the current wasm instance")
            .requires_running(),
    );
}

fn cmd_info_file(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let file = dbg.get_file()?;
    let module = file.module();

    println!("File: {}", file.file_path());
    println!("{} types", module.types().len());
    println!("{} functions", module.functions().len());
    println!("{} globals", module.globals().len());
    println!("{} tables", module.tables().len());

    {
        let mut func_count = 0;
        let mut table_count = 0;
        let mut memory_count = 0;
        let mut global_count = 0;
        for entry in module.imports() {
            match entry.external() {
                External::Function(_) => func_count += 1,
                External::Table(_) => table_count += 1,
                External::Memory(_) => memory_count += 1,
                External::Global(_) => global_count += 1,
            }
        }
        println!("{} imports", module.imports().len());
        if func_count > 0 {
            println!(
                " -> {} function imports (currently not supported)",
                func_count
            );
        }
        if table_count > 0 {
            println!(" -> {} table imports (currently not supported)", func_count);
        }
        if memory_count > 0 {
            println!(
                " -> {} memory imports (currently not supported)",
                memory_count
            );
        }
        if global_count > 0 {
            println!(
                " -> {} global imports (currently not supported)",
                global_count
            );
        }
    }

    println!("{} exports", module.exports().len());
    println!("{} linear memories", module.memories().len());

    for (i, entry) in module.memories().iter().enumerate() {
        let limits = entry.limits();
        if let Some(max) = limits.maximum() {
            println!(
                " -> Memory {:>2}: Min. 0x{:x} bytes, Max. 0x{:x} bytes",
                i,
                limits.initial() * PAGE_SIZE,
                max * PAGE_SIZE
            );
        } else {
            println!(
                " -> Memory {:>2}: Min. 0x{:x} bytes",
                i,
                limits.initial() * PAGE_SIZE
            );
        }
    }

    println!("{} table initializers", module.element_entries().len());
    println!("{} data initializers", module.data_entries().len());

    for entry in module.data_entries() {
        let offset = match entry.offset() {
            InitExpr::Const(val) => format!("{}", val.to::<u32>().unwrap()),
            InitExpr::Global(index) => format!("of global {}", index),
        };
        println!(
            " -> for memory {} at offset {} for 0x{:x} bytes",
            entry.index(),
            offset,
            entry.value().len()
        );
    }

    if !module.custom_sections().is_empty() {
        println!("{} custom sections", module.custom_sections().len());
        for custom_sec in module.custom_sections() {
            println!(
                " -> {}: {} bytes",
                custom_sec.name(),
                custom_sec.payload().len()
            );
        }
    }

    Ok(())
}

fn cmd_info_break(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
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

fn cmd_info_ip(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let ip = dbg.get_vm()?.ip();
    println!("Function: {}", ip.func_index);
    println!("Instruction: {}", ip.instr_index);
    Ok(())
}

fn cmd_info_types(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let types = dbg.get_file()?.module().types();
    println!("{} types", types.len());
    for (i, entry) in types.iter().enumerate() {
        println!("Type {:>2}: {}", i, entry);
    }
    Ok(())
}

fn cmd_info_imports(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let module = dbg.get_file()?.module();
    println!("{} imports", module.imports().len());
    for entry in module.imports() {
        match entry.external() {
            External::Function(type_index) => {
                let func_type = &module.types()[*type_index as usize];
                // TODO: group functions from the same module
                println!(
                    "fn {}.{}{}",
                    entry.module(),
                    entry.field(),
                    &func_type.to_string()[3..]
                );
            }
            External::Table(table_type) => println!("Table: {:?}", table_type),
            External::Memory(memory_type) => println!("Memory: {:?}", memory_type),
            External::Global(global_type) => println!("Global: {:?}", global_type),
        }
    }
    Ok(())
}

fn cmd_info_functions(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let functions = dbg.get_file()?.module().functions();
    let highest_index_len = functions.len().to_string().len();
    for (i, func) in functions.iter().enumerate() {
        if func.is_imported() {
            println!(" {:>2$}: imported {}", i, func, highest_index_len);
        } else {
            println!(" {:>2$}: {}", i, func, highest_index_len);
        }
    }
    Ok(())
}

fn cmd_info_tables(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    for (i, table) in dbg.get_file()?.module().tables().iter().enumerate() {
        println!(
            "Table {:>2}: {:?}, Length: {}",
            i,
            table.elem_type(),
            table.limits().initial()
        );
    }
    Ok(())
}

fn cmd_info_memory(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    for (i, entry) in dbg.get_file()?.module().memories().iter().enumerate() {
        let limits = entry.limits();
        if let Some(max) = limits.maximum() {
            println!(
                "Memory {:>2}: Min. 0x{:x} bytes, Max. 0x{:x} bytes",
                i,
                limits.initial() * PAGE_SIZE,
                max * PAGE_SIZE
            );
        } else {
            println!(
                "Memory {:>2}: Min. 0x{:x} bytes",
                i,
                limits.initial() * PAGE_SIZE
            );
        }
    }
    Ok(())
}

fn cmd_info_globals(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    for (i, global) in dbg.get_file()?.module().globals().iter().enumerate() {
        let const_str = if global.is_mutable() {
            "mut  "
        } else {
            "const"
        };
        let init_str = match global.init_expr() {
            InitExpr::Const(val) => format!("{}", val),
            InitExpr::Global(index) => format!("global {}", index),
        };
        println!(" {}: {} {:15} = {}", i, const_str, global.name(), init_str);
    }
    Ok(())
}

fn cmd_info_exports(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    // TODO: Implement
    println!("Not implemented");
    Ok(())
}

fn cmd_info_start(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let module = dbg.get_file()?.module();
    if let Some(start_func_index) = module.start_func() {
        let start_func = module.get_func(start_func_index).unwrap();
        println!("Function {}: {}", start_func_index, start_func);
    } else {
        println!("No start function declared");
    }
    Ok(())
}

fn cmd_info_elements(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    // TODO: Implement
    println!("Not implemented");
    Ok(())
}

fn cmd_info_data(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    // TODO: Implement
    println!("Not implemented");
    Ok(())
}

fn cmd_info_custom(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    // TODO: Implement
    println!("Not implemented");
    Ok(())
}

fn cmd_status(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    if let Some(trap) = dbg.get_vm()?.trap() {
        if let Trap::ExecutionFinished = trap {
            println!("Finished execution");
        } else {
            println!("Trap: {}", trap);
        }
    } else {
        println!("No trap");
        let ip = dbg.get_vm()?.ip();
        println!("Function: {}", ip.func_index);
        println!("Instruction: {}", ip.instr_index);
    }
    Ok(())
}
