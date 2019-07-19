use parity_wasm::elements::{External, Instruction, Type::Function, FunctionType};

use wasmdbg::vm::{CodePosition, Trap};
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
                Command::new("ip", cmd_info_ip)
                    .description("Print instruction pointer")
                    .requires_running(),
            )
            .add_subcommand(
                Command::new("types", cmd_info_types)
                    .description("Print type section"),
            )
            .add_subcommand(
                Command::new("imports", cmd_info_imports)
                    .description("Print import section"),
            )
            .add_subcommand(
                Command::new("functions", cmd_info_functions)
                    .description("Print function section"),
            )
            .add_subcommand(
                Command::new("tables", cmd_info_tables)
                    .description("Print table section"),
            )
            .add_subcommand(
                Command::new("memory", cmd_info_memory)
                    .description("Print memory section"),
            )
            .add_subcommand(
                Command::new("global", cmd_info_globals)
                    .description("Print global section"),
            )
            .add_subcommand(
                Command::new("export", cmd_info_exports)
                    .description("Print export section"),
            )
            .add_subcommand(
                Command::new("start", cmd_info_start)
                    .description("Print start section"),
            )
            .add_subcommand(
                Command::new("element", cmd_info_elements)
                    .description("Print element section"),
            )
            .add_subcommand(
                Command::new("data", cmd_info_data)
                    .description("Print data section"),
            )
            .add_subcommand(
                Command::new("custom", cmd_info_custom)
                    .takes_args("[INDEX:u32|NAME:str]")
                    .description("Print custom sections"),
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

fn cmd_info_file(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let file = dbg.file().unwrap();
    let module = file.module();

    println!("File: {}", file.file_path());

    match module.type_section() {
        Some(type_sec) => println!("{} types", type_sec.types().len()),
        None => println!("No type section"),
    }

    match module.import_section() {
        Some(import_sec) => {
            let mut func_count = 0;
            let mut table_count = 0;
            let mut memory_count = 0;
            let mut global_count = 0;
            for entry in import_sec.entries() {
                match entry.external() {
                    External::Function(_) => func_count += 1,
                    External::Table(_) => table_count += 1,
                    External::Memory(_) => memory_count += 1,
                    External::Global(_) => global_count += 1,
                }
            }
            println!("{} imports", import_sec.entries().len());
            if func_count > 0 {
                println!(" -> {} function imports (currently not supported)", func_count);
            }
            if table_count > 0 {
                println!(" -> {} table imports (currently not supported)", func_count);
            }
            if memory_count > 0 {
                println!(" -> {} memory imports (currently not supported)", memory_count);
            }
            if global_count > 0 {
                println!(" -> {} global imports (currently not supported)", global_count);
            }
        },
        None => println!("No import section"),
    }

    match module.function_section() {
        Some(func_sec) => println!("{} functions", func_sec.entries().len()),
        None => println!("No function section"),
    }

    match module.table_section() {
        Some(table_sec) => println!("{} tables", table_sec.entries().len()),
        None => println!("No table section"),
    }

    match module.memory_section() {
        Some(memory_sec) => {
            println!("{} linear memories", memory_sec.entries().len());
            for (i, entry) in memory_sec.entries().iter().enumerate() {
                let limits = entry.limits();
                if let Some(max) = limits.maximum() {
                    println!(" -> Memory #{}: Min. 0x{:x}, Max. 0x{:x}", i, limits.initial(), max);
                }
                else {
                    println!(" -> Memory #{}: Min. 0x{:x}", i, limits.initial());
                }
            }
        },
        None => println!("No memory section"),
    }

    match module.global_section() {
        Some(global_sec) => println!("{} globals", global_sec.entries().len()),
        None => println!("No global section"),
    }

    match module.export_section() {
        Some(export_sec) => println!("{} exports", export_sec.entries().len()),
        None => println!("No export section"),
    }

    match module.start_section() {
        Some(start_sec) => println!("Start function: #{}", start_sec),
        None => println!("No start section"),
    }

    match module.elements_section() {
        Some(element_sec) => println!("{} table initializers", element_sec.entries().len()),
        None => println!("No elements section"),
    }

    match module.data_section() {
        Some(data_sec) => {
            println!("{} data initializers", data_sec.entries().len());
            for entry in data_sec.entries() {
                let offset = match entry.offset() {
                    Some(offset) => match offset.code().get(0) {
                        Some(Instruction::GetGlobal(index)) => format!("of global {}", index),
                        Some(Instruction::I32Const(value)) => format!("{}", value),
                        _ => String::from("0"),
                    }
                    None => String::from("0"),
                };
                println!(" -> for memory {} at offset {} for 0x{:x} bytes", entry.index(), offset, entry.value().len());
            }
        },
        None => println!("No data section"),
    }

    match module.names_section() {
        Some(_) => println!("Name section present"),
        None => println!("No name section"),
    }

    if module.custom_sections().next().is_some() {
        println!("{} custom sections", module.custom_sections().count());
        for custom_sec in module.custom_sections() {
            println!(" -> {}: {} bytes", custom_sec.name(), custom_sec.payload().len());
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
    let ip = dbg.vm().unwrap().ip();
    println!("Function: {}", ip.func_index);
    println!("Instruction: {}", ip.instr_index);
    Ok(())
}

fn cmd_info_types(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    match dbg.get_file()?.module().type_section() {
        Some(sec) => {
            println!("{} types", sec.types().len());
            for (i, entry) in sec.types().iter().enumerate() {
                let Function(func_type) = entry;
                println!("Type {:>2}: {}", i, func_type_str(func_type));
            }
        }
        None => println!("No type section"),
    }
    Ok(())
}

fn cmd_info_imports(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    let module = dbg.get_file()?.module();
    match module.import_section() {
        Some(sec) => {
            println!("{} imports", sec.entries().len());
            for entry in sec.entries() {
                match entry.external() {
                    External::Function(type_index) => {
                        let Function(func_type) = &module.type_section().unwrap().types()[*type_index as usize];
                        // TODO: group functions from the same module
                        println!("Function {}\t{:<20}\twith type {:>3}: {}", entry.module(), entry.field(), type_index, func_type_str(func_type));
                    },
                    External::Table(table_type) => println!("Table: {:?}", table_type),
                    External::Memory(memory_type) => println!("Memory: {:?}", memory_type),
                    External::Global(global_type) => println!("Global: {:?}", global_type),
                }
            }
        }
        None => println!("No import section"),
    }
    Ok(())
}

fn cmd_info_functions(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_tables(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_memory(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_globals(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_exports(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_start(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_elements(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_data(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_info_custom(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    println!("Not implemented");
    Ok(())
}

fn cmd_status(dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
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

fn func_type_str(func_type: &FunctionType) -> String {
    let params = func_type.params().iter().map(|t| t.to_string()).collect::<Vec<String>>().join(", ");
    let return_type = match func_type.return_type() {
        Some(return_type) => return_type.to_string(),
        None => String::from("()"),
    };
    format!("fn ({}) -> {}", params, return_type)
}
