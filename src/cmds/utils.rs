use std::process;

use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("python", cmd_python)
            .alias("pi")
            .takes_args("EXPR:str...")
            .description("Run python interpreter"),
    );
}

fn cmd_python(_dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let mut cmd = &mut process::Command::new("python3");
    if !args.is_empty() {
        let expr: String = args.iter().map(|expr| expr.as_str()).collect();
        let code = format!("print({})", expr);
        cmd = cmd.arg("-c").arg(code);
    }
    cmd.spawn()?.wait()?;
    Ok(())
}
