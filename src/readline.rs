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

fn complete<'a, I>(
    cmds: &Commands,
    curr_word: &'a str,
    other_words: &mut I,
) -> Option<Vec<Completion>>
where
    I: Iterator<Item = &'a str> + Clone,
{
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

fn complete_cmd_arg<'a, I>(
    arg_type: &CmdArgType,
    curr_word: &'a str,
    other_words: &mut I,
) -> (bool, Option<Vec<Completion>>)
where
    I: Iterator<Item = &'a str> + Clone,
{
    match arg_type {
        CmdArgType::Str(_)
        | CmdArgType::Fmt(_)
        | CmdArgType::Line(_)
        | CmdArgType::Usize(_)
        | CmdArgType::U32(_)
        | CmdArgType::Addr(_) => {
            if other_words.next().is_some() {
                (true, None)
            } else {
                (false, None)
            }
        }
        CmdArgType::Path(_) => {
            if other_words.next().is_some() {
                (true, None)
            } else {
                (true, Some(complete_path(curr_word)))
            }
        }

        CmdArgType::Const(val) => {
            if let Some(word) = other_words.next() {
                (*val == word, None)
            } else if val.starts_with(curr_word) {
                (true, Some(vec![Completion::simple(val.to_string())]))
            } else {
                (false, None)
            }
        }
        CmdArgType::Union(elements) => {
            let mut matches = Vec::new();
            for e in elements {
                let mut clone = other_words.clone();
                match complete_cmd_arg(e, curr_word, &mut clone) {
                    (true, Some(result)) => matches.extend(result),
                    (true, None) => {
                        if matches.is_empty() {
                            return complete_cmd_arg(e, curr_word, other_words);
                        } else {
                            return (true, Some(matches));
                        }
                    }
                    (false, _) => (),
                }
            }
            (true, Some(matches))
        }
        CmdArgType::List(_) => loop {
            match complete_cmd_arg(arg_type, curr_word, other_words) {
                (true, None) => (),
                other => return other,
            }
        },
        CmdArgType::Opt(arg_types) => {
            for arg_type in arg_types {
                match complete_cmd_arg(arg_type, curr_word, other_words) {
                    (true, None) => (),
                    other => return other,
                }
            }
            (true, None)
        }
    }
}

fn complete_cmd_args<'a, I>(
    arg_types: &[CmdArgType],
    curr_word: &'a str,
    other_words: &mut I,
) -> Option<Vec<Completion>>
where
    I: Iterator<Item = &'a str> + Clone,
{
    for arg_type in arg_types {
        match complete_cmd_arg(arg_type, curr_word, other_words) {
            (true, Some(result)) => return Some(result),
            (true, None) => (),
            (false, _) => return None,
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
