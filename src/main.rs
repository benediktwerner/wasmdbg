extern crate clap;
extern crate wasmdbg;

use clap::{App, Arg};

use std::io::{self, BufRead, Write};
use wasmdbg::Debugger;


fn print_help() {
    println!("Help not yet implemented.");
}


fn main() {
    let matches = App::new("wasmdbg")
        .version("0.1.0")
        .arg(Arg::with_name("file").help("The wasm binary to debug"))
        .get_matches();

    let mut debugger = Debugger::new();
    if let Some(file_path) = matches.value_of("file") {
        debugger.load_file(file_path).unwrap();
    }

    loop {
        print!("wasmdbg> ");
        io::stdout().flush().unwrap();

        if let Some(line) = io::stdin().lock().lines().next() {
            match line.unwrap().as_ref() {
                "test" => println!("Hello!"),
                "help" => print_help(),
                "quit" | "exit" => break,
                "" => (),
                cmd => println!("Unknown command: \"{}\". Try \"help\".", cmd),
            }
        } else {
            break;
        }
    }

    println!("Bye.");
}
