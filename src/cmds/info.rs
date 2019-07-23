use wasmdbg::vm::{CodePosition, Trap};
use wasmdbg::wasm::{External, InitExpr, PAGE_SIZE};
use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};

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
                    .description("Print breakpoints"),
            )
            .add_subcommand(
                Command::new("ip", cmd_info_ip).description("Print instruction pointer"),
            )
            .add_subcommand(Command::new("types", cmd_info_types).description("Print type section"))
            .add_subcommand(
                Command::new("imports", cmd_info_imports).description("Print import section"),
            )
            .add_subcommand(
                Command::new("functions", cmd_info_functions).alias("funcs").description("Print function section"),
            )
            .add_subcommand(
                Command::new("tables", cmd_info_tables).description("Print table section"),
            )
            .add_subcommand(
                Command::new("memory", cmd_info_memory).description("Print memory section"),
            )
            .add_subcommand(
                Command::new("global", cmd_info_globals).description("Print global section"),
            )
            .add_subcommand(
                Command::new("export", cmd_info_exports).description("Print export section"),
            )
            .add_subcommand(
                Command::new("start", cmd_info_start).description("Print start section"),
            )
            .add_subcommand(
                Command::new("element", cmd_info_elements).description("Print element section"),
            )
            .add_subcommand(Command::new("data", cmd_info_data).description("Print data section"))
            .add_subcommand(
                Command::new("custom", cmd_info_custom)
                    .takes_args("[INDEX:u32|NAME:str]")
                    .description("Print custom sections"),
            )
            .alias("i")
            .description("Print info about the programm being debugged"),
    );
    commands.add(
        Command::new("status", cmd_status).description("Print status of the current wasm instance"),
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
            println!(" -> Memory {:>2}: Min. 0x{:x} bytes", i, limits.initial() * PAGE_SIZE);
        }
    }


    match module.start_func() {
        Some(start_func) => println!("Start function: #{}", start_func),
        None => println!("No start section"),
    }

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
    for func in dbg.get_file()?.module().functions() {
        if func.is_imported() {
        println!("imported {}", func);
        }
        else {
            println!("{}", func);
        }
    }
    Ok(())
}

fn cmd_info_tables(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    for (i, table) in dbg.get_file()?.module().tables().iter().enumerate() {
        println!("Table {:>2}: {:?}", i, table.elem_type());
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
            println!("Memory {:>2}: Min. 0x{:x} bytes", i, limits.initial() * PAGE_SIZE);
        }
    }
    Ok(())
}

fn cmd_info_globals(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_exports(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_start(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_elements(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_data(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_custom(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
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
