# wasmdbg

`wasmdbg` is a gdb-like debugger for WebAssembly binaries written in Rust. It currently supports all MVP version 1 binaries as well as a (currently very limited) subset of WASI.

## Building and Installation

Building `wasmdbg` requires a [Rust Installation](https://www.rust-lang.org/).

To build and install `wasmdbg`:

```
$ git clone https://github.com/benediktwerner/wasmdbg
$ cargo install --path wasmdbg
$ wasmdbg --version
```

## Features
- Run MVP version 1 binaries
- Limited subset of WASI (currently only `wasi_unstable.proc_exit`)
- Call a specific functions with any arguments
- Read function and global names from export section
- Specify startup commands in a `.wasmdbg_init` file
- Breakpoints
- Single-stepping
- Step-over function, Step-out of function
- View disassembly
- View program state (locals, globals, memory, value stack, call stack, label stack)
- Modify program state (memory and value stack)
- Print info about the binary
- Print wasm sections
- Run a python interpreter

To view all available commands use the `help` command.
To learn more about a specific command use `help COMMAND`.
