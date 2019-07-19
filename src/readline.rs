extern crate linefeed;
extern crate shellexpand;

use std::io;
use std::sync::Arc;

use colored::*;

use linefeed::complete::{complete_path, Completer, Completion};
use linefeed::terminal::{DefaultTerminal, Terminal};
use linefeed::{Interface, Prompter, ReadResult};

use crate::cmds::{CmdArgType, Command, Commands};

lazy_static! {
    static ref HISTORY_FILE_PATH: String = shellexpand::tilde("~/.wasmdbg_history").to_string();
}

fn find_cmds<'a>(cmds: &'a Commands, prefix: &str) -> Vec<&'a Command> {
    cmds.iter()
        .filter(|cmd| {
            cmd.name.starts_with(prefix)
                || cmd.aliases.iter().any(|&alias| alias.starts_with(prefix))
        })
        .collect()
}

struct MyCompleter {
    cmds: Arc<Commands>,
}

impl<Term: Terminal> Completer<Term> for MyCompleter {
    fn complete(
        &self,
        curr_word: &str,
        prompter: &Prompter<Term>,
        start: usize,
        _end: usize,
    ) -> Option<Vec<Completion>> {
        let line = prompter.buffer();
        let mut words = line[..start].split_whitespace();
        complete(&self.cmds, curr_word, &mut words)
    }
}

fn complete(
    cmds: &Commands,
    curr_word: &str,
    other_words: &mut Iterator<Item = &str>,
) -> Option<Vec<Completion>> {
    match other_words.next() {
        Some(word) => match cmds.find_by_name(word) {
            Some(cmd) if cmd.is_subcommand() => complete(&cmd.subcommands, curr_word, other_words),
            Some(cmd) if cmd.name == "help" => {
                if other_words.next().is_some() {
                    None
                } else {
                    Some(
                        find_cmds(cmds, curr_word)
                            .iter()
                            .map(|cmd| Completion::simple(cmd.name.to_string()))
                            .collect(),
                    )
                }
            }
            Some(cmd) => complete_cmd_args(&cmd.args, curr_word, other_words),
            _ => None,
        },
        None => Some(
            find_cmds(cmds, curr_word)
                .iter()
                .map(|cmd| Completion::simple(cmd.name.to_string()))
                .collect(),
        ),
    }
}

fn match_cmd_arg(arg_type: &CmdArgType, word: &str) -> bool {
    match arg_type {
        CmdArgType::Str(_) => true,
        CmdArgType::Fmt(_) => true,
        CmdArgType::Path(_) => true,
        CmdArgType::Usize(_) => true,
        CmdArgType::U32(_) => true,
        CmdArgType::Const(val) => *val == word,
        CmdArgType::Union(elements) => elements.iter().any(|e| match_cmd_arg(e, word)),
        CmdArgType::List(arg_type) => match_cmd_arg(arg_type, word),
        CmdArgType::Opt(arg_type) => match_cmd_arg(arg_type, word),
    }
}

fn complete_cmd_arg(arg_type: &CmdArgType, word: &str) -> Option<Vec<Completion>> {
    match arg_type {
        CmdArgType::Str(_) => None,
        CmdArgType::Fmt(_) => None,
        CmdArgType::Path(_) => Some(complete_path(word)),
        CmdArgType::Usize(_) => None,
        CmdArgType::U32(_) => None,
        CmdArgType::Const(val) => {
            if val.starts_with(word) {
                Some(vec![Completion::simple(val.to_string())])
            } else {
                None
            }
        }
        CmdArgType::Union(elements) => Some(
            elements
                .iter()
                .filter_map(|e| complete_cmd_arg(e, word))
                .flatten()
                .collect(),
        ),
        CmdArgType::List(arg_type) => complete_cmd_arg(arg_type, word),
        CmdArgType::Opt(arg_type) => complete_cmd_arg(arg_type, word),
    }
}

fn complete_cmd_args(
    args: &[CmdArgType],
    curr_word: &str,
    other_words: &mut Iterator<Item = &str>,
) -> Option<Vec<Completion>> {
    for arg_type in args {
        if let Some(word) = other_words.next() {
            if !match_cmd_arg(arg_type, word) {
                return None;
            }
        } else {
            return complete_cmd_arg(arg_type, curr_word);
        }
    }
    None
}

pub struct Readline {
    interface: Interface<DefaultTerminal>,
}

impl Readline {
    pub fn new(cmds: Arc<Commands>) -> Self {
        let interface = Interface::new("wasmdbg").unwrap();
        interface.set_completer(Arc::new(MyCompleter { cmds }));
        interface
            .set_prompt(&"wasmdbg> ".red().to_string())
            .unwrap();

        if let Err(error) = interface.load_history(&*HISTORY_FILE_PATH) {
            if error.kind() != io::ErrorKind::NotFound {
                println!("Error while loading command history: {:?}", error);
            }
        }

        Readline { interface }
    }

    pub fn readline(&mut self) -> Option<String> {
        loop {
            match self.interface.read_line() {
                Ok(result) => match result {
                    ReadResult::Input(line) => {
                        if line != "" {
                            self.interface.add_history_unique(line.clone());
                        }
                        return Some(line);
                    }
                    ReadResult::Eof => return None,
                    _ => (),
                },
                Err(error) => {
                    println!("Error on readline: {}", error);
                    return None;
                }
            }
        }
    }
}

impl Drop for Readline {
    fn drop(&mut self) {
        if let Err(error) = self.interface.save_history(&*HISTORY_FILE_PATH) {
            println!("Error while saving command history: {}", error);
        }
    }
}
