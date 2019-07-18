extern crate terminal_size;
extern crate wasmdbg;

use std::fs::File;
use std::io::{self, BufRead};
use std::ops::RangeInclusive;
use std::sync::Arc;

use colored::*;
use failure::Error;
use parity_wasm::elements::{Instruction, Type::Function};
use terminal_size::{terminal_size, Width};

use wasmdbg::value::Value;
use wasmdbg::vm::{CodePosition, Trap};
use wasmdbg::{Debugger, LoadError};

type CmdResult = Result<(), Error>;

pub struct Command {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub description: Option<&'static str>,
    pub help: Option<&'static str>,
    pub requires_file: bool,
    pub requires_running: bool,
    pub argc: RangeInclusive<usize>,
    pub handler: Option<fn(&mut Debugger, &[&str]) -> CmdResult>,
    pub subcommands: Commands,
}

impl Command {
    pub fn new(name: &'static str, handler: fn(&mut Debugger, &[&str]) -> CmdResult) -> Command {
        Command {
            name,
            handler: Some(handler),
            aliases: Vec::new(),
            description: None,
            help: None,
            argc: 0..=0,
            requires_file: false,
            requires_running: false,
            subcommands: Commands {
                commands: Vec::with_capacity(0),
            },
        }
    }

    pub fn new_subcommand(name: &'static str) -> Command {
        Command {
            name,
            handler: None,
            aliases: Vec::new(),
            description: None,
            help: None,
            argc: 0..=0,
            requires_file: false,
            requires_running: false,
            subcommands: Commands {
                commands: Vec::new(),
            },
        }
    }

    pub fn is_subcommand(&self) -> bool {
        self.handler.is_none()
    }

    fn add_subcommand(mut self, cmd: Command) -> Self {
        assert!(
            self.is_subcommand(),
            "Tried to add subcommand to a command with a set handler"
        );
        self.subcommands.commands.push(cmd);
        self
    }

    pub fn handle(&self, dbg: &mut Debugger, args: &[&str]) {
        if self.requires_file && dbg.file().is_none() {
            println!("No wasm binary loaded.\nUse the \"load\" command to load one.");
            return;
        }
        if self.requires_running && dbg.vm().is_none() {
            println!("The binary is not being run.");
            return;
        }
        if let Some(handler) = self.handler {
            if !self.argc.contains(&args.len()) {
                if *self.argc.end() == 0 {
                    println!("\"{}\" takes no arguments", self.name);
                } else if (self.argc.end() - self.argc.start()) == 1 {
                    println!(
                        "\"{}\" takes exactly {} args but got {}",
                        self.name,
                        self.argc.start(),
                        args.len()
                    );
                } else {
                    println!(
                        "\"{}\" takes between {} and {} args but got {}",
                        self.name,
                        self.argc.start(),
                        self.argc.end() - 1,
                        args.len()
                    );
                }
                return;
            }
            if let Err(error) = handler(dbg, args) {
                println!("Error: {}", error);
            }
        } else if let Some(name) = args.first() {
            let cmds: &[Command] = &self.subcommands;
            for cmd in cmds {
                if cmd.has_name(name) {
                    cmd.handle(dbg, &args[1..]);
                    return;
                }
            }
            println!("Invalid subcommand: \"{}\"", name);
        } else {
            println!("This command must be followed by a subcommand:\n");
            for cmd in self.subcommands.iter() {
                match cmd.description {
                    Some(description) => println!("{} - {}", cmd.names(), description),
                    None => println!("{}", cmd.names()),
                }
            }
        }
    }

    pub fn names(&self) -> String {
        if self.aliases.is_empty() {
            self.name.to_string()
        } else {
            format!("{}, {}", self.name, self.aliases.join(", "))
        }
    }

    pub fn has_name(&self, name: &str) -> bool {
        self.name == name || self.aliases.iter().any(|&x| x == name)
    }

    pub fn alias(mut self, alias: &'static str) -> Self {
        self.aliases.push(alias);
        self
    }

    pub fn description(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }

    pub fn takes_args(mut self, argc: usize) -> Self {
        self.argc = argc..=argc + 1;
        self
    }

    pub fn takes_args_range(mut self, argc: RangeInclusive<usize>) -> Self {
        self.argc = argc;
        self
    }

    pub fn requires_file(mut self) -> Self {
        self.requires_file = true;
        self
    }

    pub fn requires_running(mut self) -> Self {
        self.requires_running = true;
        self.requires_file()
    }
}

pub struct Commands {
    pub commands: Vec<Command>,
}

impl Commands {
    pub fn all() -> Commands {
        let mut commands = Vec::new();
        commands.push(Command::new("help", cmd_unreachable).description("Show help"));
        commands.push(
            Command::new("exit", cmd_unreachable)
                .alias("quit")
                .description("Exit wasmdbg"),
        );
        commands.push(
            Command::new("load", cmd_load)
                .takes_args(1)
                .description("Load a wasm binary")
                .help("load FILE\n\nLoad the wasm binary FILE."),
        );
        commands.push(
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
        commands.push(
            Command::new("status", cmd_status)
                .description("Print status of the current wasm instance")
                .requires_running(),
        );
        commands.push(
            Command::new("run", cmd_run)
                .alias("r")
                .description("Run the currently loaded binary")
                .requires_file(),
        );
        commands.push(
            Command::new("call", cmd_call)
                .description("Call a specific function in the current runtime context")
                .takes_args_range(1..=20)
                .requires_file(),
        );
        commands.push(
            Command::new("stack", cmd_stack)
                .description("Print the current value stack")
                .requires_running(),
        );
        commands.push(
            Command::new("continue", cmd_continue)
                .alias("c")
                .description("Continue execution after a breakpoint")
                .requires_running(),
        );
        commands.push(
            Command::new("break", cmd_break)
                .alias("b")
                .takes_args_range(1..=2)
                .description("Set a breakpoint")
                .help("break FUNC_INDEX [INSTRUCTION_INDEX]\n\nSet a breakpoint at the specified function and instruction. If no instruction is specified the breakpoint is set to the function start. When execution reaches a breakpoint it will pause")
                .requires_file(),
        );
        commands.push(
            Command::new("step", cmd_step)
                .alias("stepi")
                .alias("s")
                .alias("si")
                .takes_args_range(0..=1)
                .description("Step one instruction")
                .help("step [N]\n\nStep exactly one or if an argument is given exactly N instructions.\nUnlike \"next\" this will enter subroutine calls.")
                .requires_running()
        );
        commands.push(
            Command::new("next", cmd_next)
                .alias("nexti")
                .alias("n")
                .alias("ni")
                .takes_args_range(0..=1)
                .description("Step one instruction, but skip over subroutine calls")
                .help("next [N]\n\nStep one or if an argument is given N instructions.\nUnlike \"step\" this will skip over subroutine calls.")
                .requires_running()
        );
        commands.push(
            Command::new("delete", cmd_delete)
                .description("Delete a breakpoint")
                .help("delete BREAKPOINT_INDEX\n\nDelete the breakpoint with the specified index.")
                .takes_args(1)
                .requires_file(),
        );
        commands.push(
            Command::new("disassemble", cmd_disassemble)
                .alias("disas")
                .alias("disass")
                .takes_args_range(0..=1)
                .description("Disassemble code")
                .help("disassemble [FUNC_INDEX]\n\nDisassemble the current function or the one with the specified index.")
                .requires_file(),
        );
        commands.push(
            Command::new("context", cmd_context)
                .description("Show current execution context")
                .requires_running(),
        );

        Commands { commands }
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Command> {
        for cmd in &self.commands {
            if cmd.has_name(name) {
                return Some(cmd);
            }
        }
        None
    }
}

pub struct CommandHandler {
    commands: Arc<Commands>,
    last_line: Option<String>,
}

impl CommandHandler {
    pub fn new(commands: Arc<Commands>) -> Self {
        CommandHandler {
            commands,
            last_line: None,
        }
    }

    pub fn handle_line(&mut self, dbg: &mut Debugger, line: &str) -> bool {
        let mut args_iter = line.split_whitespace();

        if let Some(cmd_name) = args_iter.next() {
            match cmd_name {
                "help" => self.print_help(args_iter.next()),
                "quit" | "exit" => {
                    return true;
                }
                _ => match self.commands.find_by_name(cmd_name) {
                    Some(cmd) => cmd.handle(dbg, &args_iter.collect::<Vec<&str>>()),
                    None => println!("Unknown command: \"{}\". Try \"help\".", cmd_name),
                },
            }
        } else {
            if let Some(last_line) = self.last_line.clone() {
                self.handle_line(dbg, &last_line);
            }
            return false;
        }

        self.last_line = Some(line.to_string());
        false
    }

    pub fn load_init_file(&mut self, dbg: &mut Debugger, path: &str) {
        match File::open(path) {
            Ok(file) => {
                for line in io::BufReader::new(file).lines() {
                    match line {
                        Ok(line) => {
                            if self.handle_line(dbg, &line) {
                                return;
                            }
                        }
                        Err(error) => {
                            println!("Failed to read \"{}\": {}", path, error);
                            return;
                        }
                    }
                }
            }
            Err(ref error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => println!("Failed to open \"{}\": {}", path, error),
        }
    }

    fn print_help(&self, cmd_name: Option<&str>) {
        if let Some(cmd_name) = cmd_name {
            match self.commands.find_by_name(cmd_name) {
                Some(cmd) => println!(
                    "{}",
                    cmd.help
                        .or(cmd.description)
                        .unwrap_or("No help for this command")
                ),
                None => println!("Unknown command: \"{}\". Try \"help\".", cmd_name),
            }
        } else {
            for cmd in self.commands.iter() {
                match cmd.description {
                    Some(description) => println!("{} - {}", cmd.names(), description),
                    None => println!("{}", cmd.names()),
                }
            }
            println!("\nType \"help\" followed by a command to learn more about it.")
        }
    }
}

impl std::ops::Deref for Commands {
    type Target = [Command];

    fn deref(&self) -> &Self::Target {
        &self.commands
    }
}

pub fn load_file(dbg: &mut Debugger, file_path: &str) {
    if let Err(error) = dbg.load_file(file_path) {
        match error {
            LoadError::FileNotFound => println!("File not found: \"{}\"", file_path),
            LoadError::SerializationError(serialization_error) => {
                println!("Error while loading file: {}", serialization_error)
            }
        }
    } else {
        println!("Loaded \"{}\"", file_path);
    }
}

fn cmd_unreachable(_dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    unreachable!();
}

fn cmd_load(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    load_file(dbg, args[0]);
    Ok(())
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

fn cmd_run(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    print_run_result(dbg.run()?, dbg)
}

fn cmd_call(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let module = dbg.module().unwrap();
    let func_index = args[0].parse()?;
    let args = &args[1..];

    let func_section = module
        .function_section()
        .ok_or_else(|| format_err!("No function section found"))?;
    let func = func_section
        .entries()
        .get(func_index as usize)
        .ok_or_else(|| format_err!("No function with index {}", func_index))?;
    let func_type = func.type_ref();
    let Function(func_type) = &module.type_section().unwrap().types()[func_type as usize];

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

fn cmd_stack(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    let value_stack = dbg.vm().unwrap().value_stack();
    if value_stack.is_empty() {
        println!("<empty>");
        return Ok(());
    }
    for value in value_stack.iter().rev() {
        match value {
            Value::I32(val) => println!("int32   : {}", val),
            Value::I64(val) => println!("int64   : {}", val),
            Value::F32(val) => println!("float32 : {}", val),
            Value::F64(val) => println!("float64 : {}", val),
            Value::V128(val) => println!("v128    : {}", val),
        }
    }
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

fn cmd_break(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let func_index = args[0].parse()?;
    let instr_index = args.get(1).map(|n| n.parse()).transpose()?.unwrap_or(0);
    let breakpoint = CodePosition {
        func_index,
        instr_index,
    };
    let index = dbg.add_breakpoint(breakpoint)?;
    println!("Set breakpoit {} at {}:{}", index, func_index, instr_index);
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
    print_context(dbg)
}

fn cmd_next(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let steps: u32 = args.get(0).map(|n| n.parse()).transpose()?.unwrap_or(1);
    for _ in 0..steps {
        if let Some(trap) = dbg.next_instruction()? {
            return print_run_result(trap, dbg);
        }
    }
    print_context(dbg)
}

const DISASSEMBLY_DEFAULT_MAX_LINES: usize = 20;

fn cmd_disassemble(dbg: &mut Debugger, args: &[&str]) -> CmdResult {
    let index = match args.get(0).map(|n| n.parse()).transpose()? {
        Some(index) => index,
        _ => dbg.get_vm()?.ip().func_index,
    };
    if let Some(code) = dbg
        .get_file()?
        .module()
        .code_section()
        .and_then(|c| c.bodies().get(index))
        .map(|b| b.code().elements())
    {
        if args.is_empty() && code.len() > DISASSEMBLY_DEFAULT_MAX_LINES {
            let ip = dbg.get_vm()?.ip();
            let start = if ip.instr_index > code.len() - DISASSEMBLY_DEFAULT_MAX_LINES {
                code.len() - DISASSEMBLY_DEFAULT_MAX_LINES
            } else {
                ip.instr_index.max(2) - 2
            };
            let end = start + DISASSEMBLY_DEFAULT_MAX_LINES;
            print_disassembly(dbg, CodePosition::new(index, start), &code[start..end]);
        } else {
            print_disassembly(dbg, CodePosition::new(index, 0), code);
        }
    } else {
        bail!("Invalid function index");
    }
    Ok(())
}

fn print_disassembly(dbg: &Debugger, start: CodePosition, instrs: &[Instruction]) {
    let curr_instr_index = dbg.vm().and_then(|vm| {
        if vm.ip().func_index == start.func_index {
            Some(vm.ip().instr_index)
        } else {
            None
        }
    });
    let max_index_len = (start.instr_index + instrs.len() - 1).to_string().len();
    let breakpoints = dbg.breakpoints().ok();
    for (i, instr) in instrs.iter().enumerate() {
        let instr_index = start.instr_index + i;
        let addr_str = format!("{}:{:>02$}", start.func_index, instr_index, max_index_len);
        let breakpoint = match breakpoints {
            Some(ref breakpoints) => {
                breakpoints.find(&CodePosition::new(start.func_index, instr_index))
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

fn print_header(text: &str) {
    let terminal_width = match terminal_size() {
        Some((Width(w), _)) => w as usize,
        None => 80,
    };
    let line_length = terminal_width - text.len() - 8;
    println!(
        "{}",
        format!("──[ {} ]──{:─<2$}", text, "", line_length).blue()
    )
}

fn print_context(dbg: &mut Debugger) -> CmdResult {
    print_header("DISASM");
    cmd_disassemble(dbg, &[])?;
    print_header("STACK");
    cmd_stack(dbg, &[])
}

fn cmd_context(dbg: &mut Debugger, _args: &[&str]) -> CmdResult {
    print_context(dbg)
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
            print_context(dbg)?;
        }
        _ => println!("Trap: {}", trap),
    }
    Ok(())
}
