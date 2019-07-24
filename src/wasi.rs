use crate::vm::{VM, VMResult, Trap};

#[derive(Clone, Copy)]
pub enum WasiFunction {
    FdFdstatGet,
    FdPrestatGet,
    FdPrestatDirName,
    FdSeek,
    FdRead,
    FdWrite,
    FdClose,
    PathOpen,
    ProcExit,
    EnvironSizesGet,
    EnvironGet,
    ArgsSizesGet,
    ArgsGet,
    RandomGet,
}

impl WasiFunction {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            // "fd_fdstat_get" => WasiFunction::FdFdstatGet,
            // "fd_prestat_get" => WasiFunction::FdPrestatGet,
            // "fd_prestat_dir_name" => WasiFunction::FdPrestatDirName,
            // "fd_seek" => WasiFunction::FdSeek,
            // "fd_read" => WasiFunction::FdRead,
            // "fd_write" => WasiFunction::FdWrite,
            // "fd_close" => WasiFunction::FdClose,
            // "path_open" => WasiFunction::PathOpen,
            "proc_exit" => WasiFunction::ProcExit,
            // "environ_sizes_get" => WasiFunction::EnvironSizesGet,
            // "environ_get" => WasiFunction::EnvironGet,
            // "args_sizes_get" => WasiFunction::ArgsSizesGet,
            // "args_get" => WasiFunction::ArgsGet,
            // "random_get" => WasiFunction::RandomGet,
            _ => return None,
        })
    }

    pub fn handle(self, vm: &mut VM) -> VMResult<()> {
        match self {
            WasiFunction::FdFdstatGet => (),
            WasiFunction::FdPrestatGet => (),
            WasiFunction::FdPrestatDirName => (),
            WasiFunction::FdSeek => (),
            WasiFunction::FdRead => (),
            WasiFunction::FdWrite => (),
            WasiFunction::FdClose => (),
            WasiFunction::PathOpen => (),
            WasiFunction::ProcExit => return Err(Trap::WasiExit(vm.pop_as()?)),
            WasiFunction::EnvironSizesGet => (),
            WasiFunction::EnvironGet => (),
            WasiFunction::ArgsSizesGet => (),
            WasiFunction::ArgsGet => (),
            WasiFunction::RandomGet => (),
        }
        Ok(())
    }
}
