extern crate linefeed;
extern crate shellexpand;

use std::io;
use std::sync::Arc;

use colored::*;

use linefeed::complete::{complete_path, Completer, Completion};
use linefeed::terminal::{DefaultTerminal, Terminal};
use linefeed::{Interface, Prompter, ReadResult};

use crate::cmds::{Command, Commands};

lazy_static! {
    static ref HISTORY_FILE_PATH: String = shellexpand::tilde("~/.wasmdbg_history").to_string();
}


fn find_cmds<'a>(cmds: &'a Commands, prefix: &str) -> Vec<&'a Command> {
    cmds.commands
        .iter()
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
            Some(cmd) if *cmd.argc.start() > 0 => Some(complete_path(curr_word)),
            Some(cmd) if cmd.is_subcommand() => complete(&cmd.subcommands, curr_word, other_words),
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
