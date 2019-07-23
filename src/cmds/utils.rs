use std::process;

use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("load", cmd_load)
            .takes_args("FILE:path")
            .description("Load a wasm binary")
            .help("Load the wasm binary FILE."),
    );
    commands.add(
        Command::new("python", cmd_python)
            .alias("pi")
            .takes_args("[EXPR:line]")
            .description("Run python interpreter"),
    );
}

fn cmd_python(_dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let mut cmd = &mut process::Command::new("python3");
    if !args.is_empty() {
        let expr: String = args.iter().map(|expr| expr.as_string()).collect();
        let code = format!("print({})", expr);
        cmd = cmd.arg("-c").arg(code);
    }
    cmd.spawn()?.wait()?;
    Ok(())
}

fn cmd_load(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let file_path = &args[0].as_string();
    if let Err(error) = dbg.load_file(file_path) {
        println!("{}", error);
    } else {
        println!("Loaded \"{}\"", file_path);
    }
    Ok(())
}
