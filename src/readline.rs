extern crate rustyline;
extern crate shellexpand;

use std::io::ErrorKind;

use colored::*;
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::{CompletionType, Config, Context, Editor, Helper};

use crate::cmds::Commands;

lazy_static! {
    static ref HISTORY_FILE_PATH: String = shellexpand::tilde("~/.wasmdbg_history").to_string();
}

fn pair_from_string(s: &'static str) -> Pair {
    let mut replacement = s.to_string();
    replacement.push(' ');
    Pair {
        display: s.to_string(),
        replacement,
    }
}

struct MyHelper<'a> {
    filename_completer: FilenameCompleter,
    cmds: &'a Commands,
}

impl<'a> MyHelper<'a> {
    fn new(cmds: &'a Commands) -> Self {
        MyHelper {
            filename_completer: FilenameCompleter::new(),
            cmds,
        }
    }
}

impl<'a> Completer for MyHelper<'a> {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut words = line.split_whitespace();

        match words.next() {
            Some(word) => {
                let mut canidates = Vec::new();
                for cmd in &self.cmds.commands {
                    if cmd.name.starts_with(word)
                        || cmd.aliases.iter().any(|&alias| alias.starts_with(word))
                    {
                        canidates.push(cmd);
                    }
                }
                if words.count() > 0 || line[..pos].chars().last().unwrap().is_whitespace() {
                    match canidates.pop() {
                        Some(cmd) if cmd.argc > 0 => {
                            self.filename_completer.complete(line, pos, ctx)
                        }
                        _ => Ok((0, Vec::new())),
                    }
                } else {
                    Ok((
                        0,
                        canidates
                            .iter()
                            .map(|cmd| pair_from_string(cmd.name))
                            .collect(),
                    ))
                }
            }
            None => Ok((
                0,
                self.cmds
                    .commands
                    .iter()
                    .map(|cmd| pair_from_string(cmd.name))
                    .collect(),
            )),
        }
    }
}

impl<'a> Hinter for MyHelper<'a> {}
impl<'a> Highlighter for MyHelper<'a> {}
impl<'a> Helper for MyHelper<'a> {}

pub struct Readline<'a> {
    editor: Editor<MyHelper<'a>>,
}

impl<'a> Readline<'a> {
    pub fn new(cmds: &'a Commands) -> Self {
        let config = Config::builder()
            .completion_type(CompletionType::List)
            .build();
        let mut editor = Editor::with_config(config);
        let rl_helper = MyHelper::new(cmds);
        editor.set_helper(Some(rl_helper));

        if let Err(error) = editor.load_history(&*HISTORY_FILE_PATH) {
            match error {
                ReadlineError::Io(ref io_error) if io_error.kind() == ErrorKind::NotFound => (),
                _ => println!("Error while loading command history: {:?}", error),
            }
        }

        Readline { editor }
    }

    pub fn readline(&mut self) -> Option<String> {
        loop {
            match self.editor.readline(&"wasmdbg> ".red().to_string()) {
                Ok(line) => {
                    self.editor.add_history_entry(line.as_str());
                    return Some(line);
                }
                Err(ReadlineError::Interrupted) => (),
                Err(ReadlineError::Eof) => return None,
                Err(error) => println!("Error on readline: {}", error),
            }
        }
    }
}

impl<'a> Drop for Readline<'a> {
    fn drop(&mut self) {
        if let Err(error) = self.editor.save_history(&*HISTORY_FILE_PATH) {
            println!("Error while saving command history: {}", error);
        }
    }
}
