# wasmdbg

`wasmdbg` is a gdb-like debugger for WebAssembly binaries written in Rust. It currently supports all MVP version 1 binaries as well as a (currently very limited) subset of WASI.

## Building and Installation

Building and installing `wasmdbg` requires a [Rust Installation](https://www.rust-lang.org/).

To install `wasmdbg`:

```
$ git clone https://github.com/benediktwerner/wasmdbg
$ cargo install --path wasmdbg
$ wasmdbg --version
```

To build `wasmdbg`:
```
$ git clone https://github.com/benediktwerner/wasmdbg
$ cd wasmdbg
$ cargo build
$ ./target/debug/wasmdbg --version
```


## Features
- Run MVP version 1 binaries
- Limited subset of WASI (currently only `wasi_unstable.proc_exit`)
- Breakpoints: `break`
- Single-stepping: `step`
- Step-over function: `next`
- Step-out of function: `finish`
- View disassembly: `disas`
- View program state: `context`, `locals`, `globals`, value `stack`, `backtrace` and `labels` stack
- Modify program state: `set memory` and `set stack`)
- Print info about the binary: `info file/imports/exports/functions/tables/memory/globals/start`
- Call a specific functions with any arguments: `call`
- Automatically read function and global names from export section
- Specify startup commands in a `.wasmdbg_init` file
- Run a python interpreter: `python`

To view all available commands use the `help` command.
To learn more about a specific command use `help COMMAND`.
