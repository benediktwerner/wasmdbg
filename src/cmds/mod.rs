extern crate terminal_size;
extern crate wasmdbg;

use std::fs::File;
use std::io::{self, BufRead};
use std::sync::Arc;

use failure::Error;

use wasmdbg::Debugger;

mod context;
mod execution;
mod format;
mod info;
mod modify;
mod parse;
mod printing;
mod utils;

use format::Format;
use parse::{parse_types, ParseCmdArg};

type CmdResult = Result<(), Error>;

pub enum CmdArg {
    Str(String),
    Fmt(u32, u32, Format),
    Const(&'static str),
    Usize(usize),
    U32(u32),
}

impl CmdArg {
    fn type_str(&self) -> &'static str {
        match self {
            CmdArg::Str(_) => "str",
            CmdArg::Fmt(..) => "fmt",
            CmdArg::Const(_) => "const",
            CmdArg::Usize(_) => "usize",
            CmdArg::U32(_) => "u32",
        }
    }

    fn as_string(&self) -> String {
        match self {
            CmdArg::Str(val) => val.to_string(),
            CmdArg::Const(val) => val.to_string(),
            _ => panic!(
                "Parsed arg has wrong type. Expected str, found {}",
                self.type_str()
            ),
        }
    }

    fn as_u32(&self) -> u32 {
        match self {
            CmdArg::U32(val) => *val,
            _ => panic!(
                "Parsed arg has wrong type. Expected u32, found {}",
                self.type_str()
            ),
        }
    }

    fn as_usize(&self) -> usize {
        match self {
            CmdArg::Usize(val) => *val,
            _ => panic!(
                "Parsed arg has wrong type. Expected usize, found {}",
                self.type_str()
            ),
        }
    }
}

trait CmdArgOptionExt {
    fn as_u32_or(&self, default: u32) -> u32;
    fn as_usize_or(&self, default: usize) -> usize;
}

impl<'a> CmdArgOptionExt for Option<&'a CmdArg> {
    fn as_u32_or(&self, default: u32) -> u32 {
        match self {
            Some(arg) => arg.as_u32(),
            None => default,
        }
    }

    fn as_usize_or(&self, default: usize) -> usize {
        match self {
            Some(arg) => arg.as_usize(),
            None => default,
        }
    }
}

pub enum CmdArgType {
    Str(&'static str),
    Fmt(&'static str),
    Path(&'static str),
    Line(&'static str),
    Usize(&'static str),
    U32(&'static str),
    Addr(&'static str),
    Const(&'static str),
    Union(Vec<CmdArgType>),
    List(Box<CmdArgType>),
    Opt(Vec<CmdArgType>),
}

pub struct Command {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub description: Option<&'static str>,
    pub help: Option<&'static str>,
    pub requires_file: bool,
    pub requires_running: bool,
    pub args: Vec<CmdArgType>,
    handler: Option<fn(&mut Debugger, &[CmdArg]) -> CmdResult>,
    pub subcommands: Commands,
}

impl Command {
    fn new(name: &'static str, handler: fn(&mut Debugger, &[CmdArg]) -> CmdResult) -> Command {
        Command {
            name,
            handler: Some(handler),
            aliases: Vec::new(),
            description: None,
            help: None,
            args: Vec::new(),
            requires_file: false,
            requires_running: false,
            subcommands: Commands {
                commands: Vec::with_capacity(0),
            },
        }
    }

    fn new_subcommand(name: &'static str) -> Command {
        Command {
            name,
            handler: None,
            aliases: Vec::new(),
            description: None,
            help: None,
            args: Vec::with_capacity(0),
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
            "Tried to add subcommand to a command with a handler"
        );
        self.subcommands.add(cmd);
        self
    }

    pub fn handle(&self, dbg: &mut Debugger, args: &str) {
        if self.requires_file && dbg.file().is_none() {
            println!("No wasm binary loaded.\nUse the \"load\" command to load one.");
            return;
        }
        if self.requires_running && dbg.vm().is_none() {
            println!("The binary is not being run.");
            return;
        }
        if let Some(handler) = self.handler {
            if self.args.is_empty() && !args.trim_start().is_empty() {
                println!("\"{}\" takes no arguments", self.name);
                return;
            }
            match self.args.parse_all(args) {
                Ok(args) => {
                    let result = handler(dbg, &args);
                    if let Err(error) = result {
                        println!("Error: {}", error);
                    }
                }
                Err(msg) => println!("{}", msg),
            }
        } else {
            let mut args_iter = args.trim_start().splitn(2, char::is_whitespace);
            if let Some(name) = args_iter.next() {
                let cmds: &[Command] = &self.subcommands;
                for cmd in cmds {
                    if cmd.has_name(name) {
                        cmd.handle(dbg, args_iter.next().unwrap_or(""));
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

    pub fn takes_args(mut self, args: &'static str) -> Self {
        self.args = parse_types(args);
        self
    }
}

pub struct Commands {
    commands: Vec<Command>,
}

impl Commands {
    fn new() -> Commands {
        Commands {
            commands: Vec::new(),
        }
    }

    pub fn all() -> Commands {
        let mut cmds = Commands::new();

        cmds.add(
            Command::new("help", cmd_unreachable)
                .takes_args("[ARG]")
                .description("Show help")
                .help("Show all commands or show the help for a specified command."),
        );
        cmds.add(
            Command::new("exit", cmd_unreachable)
                .alias("quit")
                .description("Exit wasmdbg"),
        );

        info::add_cmds(&mut cmds);
        utils::add_cmds(&mut cmds);
        modify::add_cmds(&mut cmds);
        context::add_cmds(&mut cmds);
        printing::add_cmds(&mut cmds);
        execution::add_cmds(&mut cmds);

        cmds
    }

    fn add(&mut self, cmd: Command) {
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
        let mut line_iter = line
            .trim()
            .splitn(2, |c: char| c.is_whitespace() || c == '/');
        let cmd_name = line_iter.next().unwrap();

        match cmd_name {
            "help" => self.print_help(line_iter.next().unwrap_or("")),
            "quit" | "exit" => {
                return true;
            }
            "" => {
                if let Some(last_line) = self.last_line.clone() {
                    self.handle_line(dbg, &last_line);
                }
                return false;
            }
            _ => match self.commands.find_by_name(cmd_name) {
                Some(cmd) => cmd.handle(dbg, line_iter.next().unwrap_or("")),
                None => println!("Unknown command: \"{}\". Try \"help\".", cmd_name),
            },
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

    fn print_help(&self, cmd_name: &str) {
        if cmd_name.is_empty() {
            for cmd in self.commands.iter() {
                match cmd.description {
                    Some(description) => println!("{} - {}", cmd.names(), description),
                    None => println!("{}", cmd.names()),
                }
            }
            println!("\nType \"help\" followed by a command to learn more about it.");
            return;
        }

        let mut args = cmd_name.split_whitespace();
        match self.commands.find_by_name(args.next().unwrap()) {
            Some(mut cmd) => {
                while cmd.is_subcommand() {
                    if let Some(subcmd) = args.next() {
                        match cmd.subcommands.find_by_name(subcmd) {
                            Some(subcmd) => cmd = subcmd,
                            None => println!("Unknown subcommand: \"{}\".", subcmd),
                        }
                    } else {
                        break;
                    }
                }
                println!(
                    "Usage: {} {}",
                    cmd_name,
                    cmd.args
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                );
                if !cmd.aliases.is_empty() {
                    println!("Alias: {}", cmd.aliases.join(", "));
                }
                println!(
                    "\n{}",
                    cmd.help
                        .or(cmd.description)
                        .unwrap_or("No help for this command")
                );
            }
            None => println!("Unknown command: \"{}\". Try \"help\".", cmd_name),
        }
    }
}

fn cmd_unreachable(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    unreachable!();
}
