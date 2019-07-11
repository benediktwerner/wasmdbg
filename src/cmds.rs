extern crate wasmdbg;

use wasmdbg::{Debugger, LoadError};


pub struct Command {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub description: Option<&'static str>,
    pub help: Option<&'static str>,
    pub requires_file: bool,
    pub requires_running: bool,
    pub argc: usize,
    pub handler: &'static Fn(&mut Debugger, &[&str]),
}

impl Command {
    pub fn new(name: &'static str, handler: &'static Fn(&mut Debugger, &[&str])) -> Command {
        Command {
            name,
            handler,
            aliases: Vec::new(),
            description: None,
            help: None,
            argc: 0,
            requires_file: false,
            requires_running: false,
        }
    }

    pub fn handle(&self, dbg: &mut Debugger, args: &[&str]) {
        if args.len() != self.argc {
            if self.argc == 0 {
                println!("\"{}\" takes no arguments", self.name);
            } else {
                println!(
                    "\"{}\" takes exactly {} args but got {}",
                    self.name,
                    self.argc,
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
    commands: Vec<Command>,
}

impl Commands {
    pub fn new() -> Commands {
        let mut commands = Vec::new();
        commands.push(
            Command::new("load", &cmd_load)
                .takes_args(1)
                .description("Load a wasm binary")
                .help("load FILE\n\nLoad the wasm binary FILE."),
        );
        commands.push(
            Command::new("info", &cmd_info)
                .description("Print info about the currently loaded binary")
                .requires_file(),
        );
        commands.push(
            Command::new("run", &cmd_run)
                .alias("r")
                .description("Run the currently loaded binary")
                .requires_file(),
        );

        Commands { commands }
    }

    fn find_by_name(&self, name: &str) -> Option<&Command> {
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
            println!("quit/exit - Exit wasmdbg");
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

fn cmd_run(_dbg: &mut Debugger, _args: &[&str]) {
    println!("Not implemented");
}
