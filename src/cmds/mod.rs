extern crate terminal_size;
extern crate wasmdbg;

use std::fs::File;
use std::io::{self, BufRead};
use std::ops::RangeInclusive;
use std::sync::Arc;

use failure::Error;

use wasmdbg::{Debugger, LoadError};

mod context;
mod execution;
mod info;

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
            subcommands: Commands::new(),
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
        self.subcommands.add(cmd);
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
    commands: Vec<Command>,
}

impl Commands {
    pub fn new() -> Commands {
        Commands {
            commands: Vec::new(),
        }
    }

    pub fn all() -> Commands {
        let mut cmds = Commands::new();

        cmds.add(
            Command::new("help", cmd_unreachable)
                .takes_args_range(0..=1)
                .description("Show help")
                .help("help [ARG]\n\nShow all commands or show the help for a specified command."),
        );
        cmds.add(
            Command::new("exit", cmd_unreachable)
                .alias("quit")
                .description("Exit wasmdbg"),
        );
        cmds.add(
            Command::new("load", cmd_load)
                .takes_args(1)
                .description("Load a wasm binary")
                .help("load FILE\n\nLoad the wasm binary FILE."),
        );

        info::add_cmds(&mut cmds);
        context::add_cmds(&mut cmds);
        execution::add_cmds(&mut cmds);

        cmds
    }

    pub fn add(&mut self, cmd: Command) {
        self.commands.push(cmd);
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

impl std::ops::Deref for Commands {
    type Target = [Command];

    fn deref(&self) -> &Self::Target {
        &self.commands
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
