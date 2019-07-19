extern crate terminal_size;
extern crate wasmdbg;

use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use std::sync::Arc;

use failure::Error;

use wasmdbg::{Debugger, LoadError};

mod info;
mod utils;
mod context;
mod printing;
mod execution;

type CmdResult = Result<(), Error>;

#[derive(Debug)]
enum CmdArg {
    Str(String),
    Const(&'static str),
    Usize(usize),
    U32(u32),
}

impl CmdArg {
    fn parse(s: &str, arg_type: &CmdArgType) -> Result<Self, Error> {
        match arg_type {
            CmdArgType::Str(_) => Ok(CmdArg::Str(s.to_string())),
            CmdArgType::Fmt(_) => panic!("Tried to parse fmt arg with CmdArg::parse()"),
            CmdArgType::Path(_) => Ok(CmdArg::Str(s.to_string())),
            CmdArgType::Usize(_) => Ok(CmdArg::Usize(s.parse()?)),
            CmdArgType::U32(_) => Ok(CmdArg::U32(s.parse()?)),
            CmdArgType::Const(val) => {
                if *val == s {
                    Ok(CmdArg::Const(val))
                } else {
                    Err(format_err!("Expected \"{}\"", val))
                }
            }
            CmdArgType::Union(elements) => {
                for e in elements.iter() {
                    if let Ok(arg) = CmdArg::parse(s, e) {
                        return Ok(arg);
                    }
                }
                Err(format_err!("Expected {}", arg_type))
            }
            CmdArgType::List(_) => panic!("Tried to parse list arg with CmdArg::parse()"),
            CmdArgType::Opt(arg_type) => CmdArg::parse(s, arg_type),
        }
    }

    fn parse_all(
        fmt_arg: Option<&str>,
        args: &[&str],
        arg_types: &[CmdArgType],
    ) -> Result<Vec<CmdArg>, String> {
        let mut args_parsed = Vec::new();
        let mut args_iter = args.iter();
        let mut arg_types_iter = arg_types.iter();

        if let Some(CmdArgType::Fmt(_)) = arg_types.get(0) {
            arg_types_iter.next();
            if let Some(fmt_arg) = fmt_arg {
                args_parsed.push(CmdArg::Str(fmt_arg.to_string()));
            } else if let Some(arg) = args.get(0) {
                if arg.starts_with('/') {
                    args_parsed.push(CmdArg::Str(arg[1..].to_string()));
                    args_iter.next();
                }
            }
        } else if let Some(fmt_arg) = fmt_arg {
            return Err(format!("Unexpected format argument: \"{}\"", fmt_arg));
        }

        for (i, arg_type) in arg_types_iter.enumerate() {
            if let Some(arg) = args_iter.next() {
                if let CmdArgType::List(arg_type) = arg_type {
                    for arg in std::iter::once(arg).chain(args_iter) {
                        match CmdArg::parse(arg, arg_type) {
                            Ok(arg) => args_parsed.push(arg),
                            Err(error) => {
                                return Err(format!("Invalid argument: \"{}\". {}", arg, error))
                            }
                        }
                    }
                    return Ok(args_parsed);
                } else {
                    match CmdArg::parse(arg, arg_type) {
                        Ok(arg) => args_parsed.push(arg),
                        Err(error) => {
                            return Err(format!("Invalid argument: \"{}\". {}", arg, error))
                        }
                    }
                }
            } else if let CmdArgType::Opt(_) = arg_type {
            } else {
                return Err(format!("Missing {}. argument", i + 1));
            }
        }

        if args_iter.next().is_some() {
            return Err(String::from("Too many arguments."));
        }

        Ok(args_parsed)
    }

    fn type_str(&self) -> &'static str {
        match self {
            CmdArg::Str(_) => "str",
            CmdArg::Const(_) => "const",
            CmdArg::Usize(_) => "usize",
            CmdArg::U32(_) => "u32",
        }
    }

    fn as_str(&self) -> String {
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
            CmdArg::Usize(val) => *val as u32,
            _ => panic!(
                "Parsed arg has wrong type. Expected u32, found {}",
                self.type_str()
            ),
        }
    }

    fn as_usize(&self) -> usize {
        match self {
            CmdArg::U32(val) => *val as usize,
            CmdArg::Usize(val) => *val,
            _ => panic!(
                "Parsed arg has wrong type. Expected usize, found {}",
                self.type_str()
            ),
        }
    }
}

trait CmdArgOption {
    fn as_u32_or(&self, default: u32) -> u32;
    fn as_usize_or(&self, default: usize) -> usize;
}

impl<'a> CmdArgOption for Option<&'a CmdArg> {
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
    Usize(&'static str),
    U32(&'static str),
    Const(&'static str),
    Union(Vec<CmdArgType>),
    List(Box<CmdArgType>),
    Opt(Box<CmdArgType>),
}

impl fmt::Display for CmdArgType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CmdArgType::Str(name) => write!(f, "{}", name),
            CmdArgType::Fmt(name) => write!(f, "/{}", name),
            CmdArgType::Path(name) => write!(f, "{}", name),
            CmdArgType::Usize(name) => write!(f, "{}", name),
            CmdArgType::U32(name) => write!(f, "{}", name),
            CmdArgType::Const(val) => write!(f, "{}", val),
            CmdArgType::Union(elements) => write!(
                f,
                "{}",
                elements
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join("|")
            ),
            CmdArgType::List(arg) => write!(f, "{}...", arg),
            CmdArgType::Opt(arg) => write!(f, "[{}]", arg),
        }
    }
}

impl From<&'static str> for CmdArgType {
    fn from(s: &'static str) -> Self {
        if s.starts_with('[') {
            assert!(s.ends_with(']'), "Unmachted '[' in cmd args: {}", s);
            CmdArgType::Opt(Box::new(s[1..s.len() - 1].into()))
        } else if s.ends_with("...") {
            CmdArgType::List(Box::new(s[..s.len() - 3].into()))
        } else if s.contains('|') {
            CmdArgType::Union(s.split('|').map(|a| a.into()).collect())
        } else if s.contains(':') {
            let mut split = s.split(':');
            let name = split.next().unwrap();
            match split.next().unwrap() {
                "str" => CmdArgType::Str(name),
                "path" => CmdArgType::Path(name),
                "usize" => CmdArgType::Usize(name),
                "u32" => CmdArgType::U32(name),
                other => panic!("Invalid type in cmd arguments: {}", other),
            }
        } else if s.starts_with('/') {
            CmdArgType::Fmt(&s[1..])
        } else {
            CmdArgType::Const(s)
        }
    }
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
            "Tried to add subcommand to a command with a set handler"
        );
        self.subcommands.add(cmd);
        self
    }

    pub fn handle(&self, dbg: &mut Debugger, fmt_arg: Option<&str>, args: &[&str]) {
        if self.requires_file && dbg.file().is_none() {
            println!("No wasm binary loaded.\nUse the \"load\" command to load one.");
            return;
        }
        if self.requires_running && dbg.vm().is_none() {
            println!("The binary is not being run.");
            return;
        }
        if let Some(handler) = self.handler {
            if self.args.is_empty() && !args.is_empty() {
                println!("\"{}\" takes no arguments", self.name);
                return;
            }
            match CmdArg::parse_all(fmt_arg, args, &self.args) {
                Ok(args) => {
                    if let Err(error) = handler(dbg, &args) {
                        println!("Error: {}", error);
                    }
                }
                Err(msg) => println!("{}", msg),
            }
        } else if let Some(name) = args.first() {
            let cmds: &[Command] = &self.subcommands;
            for cmd in cmds {
                if cmd.has_name(name) {
                    cmd.handle(dbg, None, &args[1..]);
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

    pub fn takes_args(mut self, args: &'static str) -> Self {
        self.args = args.split_whitespace().map(|arg| arg.into()).collect();
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
        cmds.add(
            Command::new("load", cmd_load)
                .takes_args("FILE:path")
                .description("Load a wasm binary")
                .help("Load the wasm binary FILE."),
        );

        info::add_cmds(&mut cmds);
        utils::add_cmds(&mut cmds);
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
        let mut args_iter = line.split_whitespace();

        if let Some(mut cmd_name) = args_iter.next() {
            let fmt = if cmd_name.contains('/') {
                let mut split = cmd_name.split('/');
                cmd_name = split.next().unwrap();
                split.next()
            } else {
                None
            };
            match cmd_name {
                "help" => self.print_help(args_iter.next()),
                "quit" | "exit" => {
                    return true;
                }
                _ => match self.commands.find_by_name(cmd_name) {
                    Some(cmd) => cmd.handle(dbg, fmt, &args_iter.collect::<Vec<&str>>()),
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
                Some(cmd) => {
                    println!(
                        "Usage: {} {}\n",
                        cmd.name,
                        cmd.args
                            .iter()
                            .map(|a| a.to_string())
                            .collect::<Vec<String>>()
                            .join(" ")
                    );
                    println!(
                        "{}",
                        cmd.help
                            .or(cmd.description)
                            .unwrap_or("No help for this command")
                    );
                }
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

fn cmd_unreachable(_dbg: &mut Debugger, _args: &[CmdArg]) -> CmdResult {
    unreachable!();
}

fn cmd_load(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    load_file(dbg, &args[0].as_str());
    Ok(())
}
