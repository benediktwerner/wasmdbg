#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;

extern crate clap;
extern crate colored;
extern crate parity_wasm;
extern crate wasmdbg;

use std::sync::Arc;

use clap::{App, Arg};
use wasmdbg::Debugger;

mod cmds;
mod readline;

use cmds::{load_file, Commands};
use readline::Readline;


const VERSION: &str = env!("CARGO_PKG_VERSION");


fn main() {
    let matches = App::new("wasmdbg")
        .version(VERSION)
        .arg(Arg::with_name("file").help("The wasm binary to debug"))
        .get_matches();

    let mut dbg = Debugger::new();
    let cmds = Arc::new(Commands::new());
    let mut rl = Readline::new(cmds.clone());

    if let Some(file_path) = matches.value_of("file") {
        load_file(&mut dbg, file_path);
    }

    while let Some(line) = rl.readline() {
        if cmds.handle_line(&mut dbg, &line) {
            break;
        }
    }

    println!("Bye.");
}
