#[macro_use]
extern crate anyhow;

use std::sync::Arc;

use clap::{App, Arg};
use wasmdbg::Debugger;

mod cmds;
mod readline;
mod utils;

use cmds::{CommandHandler, Commands};
use readline::Readline;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let matches = App::new("wasmdbg")
        .version(VERSION)
        .arg(Arg::with_name("file").help("The wasm binary to debug"))
        .get_matches();

    let mut dbg = Debugger::new();
    let cmds = Arc::new(Commands::all());
    let mut rl = Readline::new(Arc::clone(&cmds));
    let mut cmd_handler = CommandHandler::new(cmds);

    if let Some(file_path) = matches.value_of("file") {
        if let Err(error) = dbg.load_file(file_path) {
            println!("{}", error);
        } else {
            println!("Loaded \"{}\"", file_path);
        }
    }

    cmd_handler.load_init_file(&mut dbg, ".wasmdbg_init");

    while let Some(line) = rl.readline() {
        if cmd_handler.handle_line(&mut dbg, &line) {
            break;
        }
    }

    println!("Bye.");
}
