extern crate clap;
extern crate wasmdbg;

use clap::{App, Arg};
use std::io::{self, BufRead, Write};
use wasmdbg::Debugger;

mod cmds;
use cmds::Command;


fn print_help(cmds: &Vec<Command>) {
    println!("help - Print this help.");
    println!("quit/exit - Exit wasmdbg.");
    for cmd in cmds {
        println!("{} - {}", cmd.name, cmd.help);
    }
}

fn test(dbg: &mut Debugger, args: &Vec<&str>) {
    println!("Testing");
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

    let mut commands = Vec::new();
    commands.push(Command::new("test", &test).help("This is a test command"));

    loop {
        print!("wasmdbg> ");
        io::stdout().flush().unwrap();

        if let Some(line) = io::stdin().lock().lines().next() {
            let line = line.unwrap();
            let mut args_iter = line.split_whitespace();

            if let Some(user_cmd) = args_iter.next() {
                match user_cmd {
                    "help" => print_help(&commands),
                    "quit" | "exit" => break,
                    "" => (),
                    _ => {
                        let mut cmd_found = false;
                        for cmd in &commands {
                            if user_cmd == cmd.name || cmd.abrvs.iter().any(|&x| x == user_cmd) {
                                cmd.handle(&mut debugger, &args_iter.collect());
                                cmd_found = true;
                                break;
                            }
                        }
                        if !cmd_found {
                            println!("Unknown command: \"{}\". Try \"help\".", user_cmd);
                        }
                    }
                }
            }
        } else {
            break;
        }
    }

    println!("Bye.");
}
