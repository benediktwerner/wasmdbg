extern crate wasmdbg;


use parity_wasm::elements::Type::Function;
use std::ops::Range;
use wasmdbg::value::Value;
use wasmdbg::vm::Trap;
use wasmdbg::{Debugger, DebuggerError, LoadError};


pub struct Command {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub description: Option<&'static str>,
    pub help: Option<&'static str>,
    pub requires_file: bool,
    pub requires_running: bool,
    pub argc: Range<usize>,
    pub handler: fn(&mut Debugger, &[&str]),
}

impl Command {
    pub fn new(name: &'static str, handler: fn(&mut Debugger, &[&str])) -> Command {
        Command {
            name,
            handler,
            aliases: Vec::new(),
            description: None,
            help: None,
            argc: 0..1,
            requires_file: false,
            requires_running: false,
        }
    }

    pub fn handle(&self, dbg: &mut Debugger, args: &[&str]) {
        if !self.argc.contains(&args.len()) {
            if self.argc.len() == 0 {
                println!("\"{}\" takes no arguments", self.name);
            } else if self.argc.len() == 1 {
                println!(
                    "\"{}\" takes exactly {} args but got {}",
                    self.name,
                    self.argc.start,
                    args.len()
                );
            } else {
                println!(
                    "\"{}\" takes between {} and {} args but got {}",
                    self.name,
                    self.argc.start,
                    self.argc.end - 1,
                    args.len()
                );

            }
            return;
        }
        if self.requires_file && dbg.file().is_none() {
            println!("No wasm binary loaded.\nUse the \"load\" command to load one.");
            return;
        }
        if self.requires_running && dbg.vm().is_none() {
            println!("The binary is not being run.");
            return;
        }
        (self.handler)(dbg, args);
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
        self.argc = argc..argc + 1;
        self
    }

    pub fn takes_args_range(mut self, argc: Range<usize>) -> Self {
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
    pub fn new() -> Commands {
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
            Command::new("info", cmd_info)
                .description("Print info about the currently loaded binary")
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
                .takes_args_range(1..20)
                .requires_file(),
        );
        commands.push(
            Command::new("stack", cmd_stack)
                .description("Print the current value stack")
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

    pub fn run_line(&self, dbg: &mut Debugger, line: &str) -> bool {
        let mut args_iter = line.split_whitespace();

        if let Some(cmd_name) = args_iter.next() {
            match cmd_name {
                "help" => self.print_help(args_iter.next()),
                "quit" | "exit" => {
                    return true;
                }
                "" => (),
                _ => match self.find_by_name(cmd_name) {
                    Some(cmd) => cmd.handle(dbg, &args_iter.collect::<Vec<&str>>()),
                    None => println!("Unknown command: \"{}\". Try \"help\".", cmd_name),
                },
            }
        }

        false
    }

    fn print_help(&self, cmd_name: Option<&str>) {
        if let Some(cmd_name) = cmd_name {
            match self.find_by_name(cmd_name) {
                Some(cmd) => println!(
                    "{}",
                    cmd.help
                        .or(cmd.description)
                        .unwrap_or("No help for this command")
                ),
                None => println!("Unknown command: \"{}\". Try \"help\".", cmd_name),
            }
        } else {
            for cmd in &self.commands {
                match cmd.description {
                    Some(description) => println!("{} - {}", cmd.names(), description),
                    None => println!("{}", cmd.names()),
                }
            }
            println!("\nType \"help\" followed by a command to learn more about it.")
        }
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

fn cmd_unreachable(_dbg: &mut Debugger, _args: &[&str]) {
    unreachable!();
}

fn cmd_load(dbg: &mut Debugger, args: &[&str]) {
    load_file(dbg, args[0]);
}

fn cmd_info(dbg: &mut Debugger, _args: &[&str]) {
    let file = dbg.file().unwrap();
    let module = file.module();

    println!("File: {}", file.file_path());

    match module.function_section() {
        Some(func_sec) => println!("{} functions", func_sec.entries().len()),
        None => println!("No functions"),
    }
}

fn cmd_run(dbg: &mut Debugger, _args: &[&str]) {
    print_run_result(dbg.run());
}

fn cmd_call(dbg: &mut Debugger, args: &[&str]) {
    let module = dbg.module().unwrap();
    match args[0].parse() {
        Ok(func_index) => {
            let args = &args[1..];
            if let Some(func_section) = module.function_section() {
                if let Some(func) = func_section.entries().get(func_index as usize) {
                    let func_type = func.type_ref();
                    let Function(func_type) =
                        &module.type_section().unwrap().types()[func_type as usize];
                    if args.len() != func_type.params().len() {
                        println!("Invalid number of arguments. Function #{} takes {} args but {} were given", func_index, func_type.params().len(), args.len());
                        return;
                    }
                    let mut args_parsed = Vec::new();
                    for (arg, value_type) in args.iter().zip(func_type.params().iter()) {
                        if let Some(arg_parsed) = Value::from_str(arg, *value_type) {
                            args_parsed.push(arg_parsed);
                        } else {
                            println!("Failed to parse argument \"{}\" as {}", arg, value_type);
                            return;
                        }
                    }
                    let has_result = func_type.return_type().is_some();
                    if print_run_result(dbg.call(func_index, &args_parsed)) && has_result {
                        println!(" => {:?}", dbg.vm().unwrap().value_stack()[0]);
                    }
                } else {
                    println!("No function with index {}", func_index);
                }
            } else {
                println!("No function section found");
            }
        }
        Err(error) => println!("Failed to parse function index: {}", error),
    }
}

fn cmd_stack(dbg: &mut Debugger, _args: &[&str]) {
    for value in dbg.vm().unwrap().value_stack() {
        match value {
            Value::I32(val) => println!("int32   : {}", val),
            Value::I64(val) => println!("int64   : {}", val),
            Value::F32(val) => println!("float32 : {}", val),
            Value::F64(val) => println!("float64 : {}", val),
            Value::V128(val) => println!("v128    : {}", val),
        }
    }
}

fn cmd_status(dbg: &mut Debugger, _args: &[&str]) {
    if let Some(trap) = dbg.vm().unwrap().trap() {
        if let Trap::ExecutionFinished = trap {
            println!("Finished execution");
        } else {
            println!("Trap: {}", trap);
        }
    } else {
        println!("No trap");
    }
}


fn print_run_result(result: Result<Trap, DebuggerError>) -> bool {
    match result {
        Ok(trap) => {
            if let Trap::ExecutionFinished = trap {
                println!("Finished execution");
                return true;
            } else {
                println!("Trap: {}", trap);
            }
        }
        Err(error) => {
            println!("Error: {}", error);
        }
    }
    false
}
