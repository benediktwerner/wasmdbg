use crate::vm::{Trap, VMResult, VM};

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
            "fd_write" => WasiFunction::FdWrite,
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
            WasiFunction::FdWrite => {
                let fd = vm.locals()?[0].to::<u32>().unwrap();
                let iovs = vm.locals()?[1].to::<u32>().unwrap();
                let iovs_len = vm.locals()?[2].to::<u32>().unwrap();
                let nwritten_out = vm.locals()?[3].to::<u32>().unwrap();
                let mut nwritten = 0;
                if fd != 1 {
                    panic!("wasi.fd_write call with fd != stdout");
                }
                for i in 0..iovs_len {
                    let iov = iovs + i * 8;
                    let str_addr: u32 = vm.memory().load(iov)?;
                    let len: u32 = vm.memory().load(iov + 4)?;
                    let start = str_addr as usize;
                    let end = (str_addr + len) as usize;
                    let s = &vm.memory().data()[start..end];
                    print!("{}", String::from_utf8(s.to_vec()).unwrap());
                    nwritten += len;
                }
                let errno: u32 = 0;
                vm.push(errno.into())?;
                vm.memory_mut().store(nwritten_out, nwritten)?;
            }
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
