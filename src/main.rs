extern crate clap;
extern crate wasmdbg;
extern crate colored;

use clap::{App, Arg};
use std::io::{self, BufRead, Write};
use wasmdbg::Debugger;
use colored::*;

mod cmds;
use cmds::Commands;


fn main() {
    let matches = App::new("wasmdbg")
        .version("0.1.0")
        .arg(Arg::with_name("file").help("The wasm binary to debug"))
        .get_matches();

    let mut dbg = Debugger::new();
    if let Some(file_path) = matches.value_of("file") {
        dbg.load_file(file_path).unwrap();
        println!("Loaded \"{}\"", file_path);
    }

    let commands = Commands::new();

    loop {
        print!("{}", "wasmdbg> ".red());
        io::stdout().flush().unwrap();

        if let Some(line) = io::stdin().lock().lines().next() {
            if commands.run_line(&mut dbg, &line.unwrap()) {
                break;
            }
        } else {
            break;
        }
    }

    println!("Bye.");
}
