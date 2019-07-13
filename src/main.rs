extern crate clap;
extern crate colored;
extern crate parity_wasm;
extern crate wasmdbg;

use clap::{App, Arg};
use colored::*;
use std::io::{self, BufRead, Write};
use wasmdbg::Debugger;

mod cmds;
use cmds::{Commands, load_file};


const VERSION: &str = env!("CARGO_PKG_VERSION");


fn main() {
    let matches = App::new("wasmdbg")
        .version(VERSION)
        .arg(Arg::with_name("file").help("The wasm binary to debug"))
        .get_matches();

    let mut dbg = Debugger::new();
    let cmds = Commands::new();

    if let Some(file_path) = matches.value_of("file") {
        load_file(&mut dbg, file_path);
    }

    loop {
        print!("{}", "wasmdbg> ".red());
        io::stdout().flush().unwrap();

        if let Some(line) = io::stdin().lock().lines().next() {
            if cmds.run_line(&mut dbg, &line.unwrap()) {
                break;
            }
        } else {
            break;
        }
    }

    println!("Bye.");
}
