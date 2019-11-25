use bwasm::{ResizableLimits, PAGE_SIZE};

use super::{eval_init_expr, InitError, Trap, VMResult};
use crate::value::LittleEndianConvert;

pub const MEMORY_MAX_PAGES: u32 = 0x10000;

pub struct Memory {
    data: Vec<u8>,
    limits: ResizableLimits,
}

impl Memory {
    pub fn new(memory: &bwasm::Memory) -> Memory {
        Memory {
            data: vec![0; memory.limits().initial() as usize],
            limits: *memory.limits(),
        }
    }

    pub fn from_module(module: &bwasm::Module) -> Result<Vec<Memory>, InitError> {
        let mut memories: Vec<_> = module.memories().iter().map(Memory::new).collect();

        for init in module.memory_inits() {
            let memory = &mut memories[init.index() as usize];
            let offset = eval_init_expr(init.offset())?;
            let offset = match offset.to::<u32>() {
                Some(val) => val as usize,
                None => return Err(InitError::OffsetInvalidType(offset.value_type())),
            };
            let len = init.data().len();
            if offset + len > memory.data.len() {
                memory.data.resize(offset + len, 0);
            }
            memory.data[offset..offset + len].copy_from_slice(init.data());
        }

        Ok(memories)
    }

    pub fn page_count(&self) -> u32 {
        (self.data.len() as u32 / PAGE_SIZE)
    }

    pub fn grow(&mut self, delta: u32) -> i32 {
        let page_count = self.page_count();
        if let Some(max) = self.limits.maximum() {
            if page_count + delta > max {
                return -1i32;
            }
        } else if page_count + delta > MEMORY_MAX_PAGES {
            return -1i32;
        }
        self.data.resize(((page_count + delta) * PAGE_SIZE) as usize, 0);
        page_count as i32
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn load<T: LittleEndianConvert>(&self, address: u32) -> VMResult<T> {
        let size = core::mem::size_of::<T>();
        let address = address as usize;
        let bytes = self
            .data
            .get(address..address + size)
            .ok_or_else(|| Trap::MemoryAccessOutOfRange((address + size) as u32))?;
        Ok(T::from_little_endian(bytes))
    }

    pub fn store<T: LittleEndianConvert>(&mut self, address: u32, value: T) -> VMResult<()> {
        let size = core::mem::size_of::<T>();
        let address = address as usize;
        let bytes = self
            .data
            .get_mut(address..address + size)
            .ok_or_else(|| Trap::MemoryAccessOutOfRange((address + size) as u32))?;
        value.to_little_endian(bytes);
        Ok(())
    }
}
