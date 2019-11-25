use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter;

use crate::vm::CodePosition;

#[derive(Clone, Copy)]
pub enum BreakpointTrigger {
    Read,
    Write,
    ReadWrite,
}

impl BreakpointTrigger {
    fn is_read(self) -> bool {
        match self {
            BreakpointTrigger::Read | BreakpointTrigger::ReadWrite => true,
            BreakpointTrigger::Write => false,
        }
    }
    fn is_write(self) -> bool {
        match self {
            BreakpointTrigger::Write | BreakpointTrigger::ReadWrite => true,
            BreakpointTrigger::Read => false,
        }
    }
}

impl fmt::Display for BreakpointTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BreakpointTrigger::Read => write!(f, "read"),
            BreakpointTrigger::Write => write!(f, "write"),
            BreakpointTrigger::ReadWrite => write!(f, "read/write"),
        }
    }
}

pub enum Breakpoint {
    Code(CodePosition),
    Memory(BreakpointTrigger, u32),
    Global(BreakpointTrigger, u32),
}

#[derive(Default)]
pub struct Breakpoints {
    code: HashSet<CodePosition>,
    memory_read: HashSet<u32>,
    memory_write: HashSet<u32>,
    global_read: HashSet<u32>,
    global_write: HashSet<u32>,
    index_map: HashMap<u32, Breakpoint>,
    next_index: u32,
}

impl Breakpoints {
    pub fn new() -> Self {
        Breakpoints {
            code: HashSet::new(),
            memory_read: HashSet::new(),
            memory_write: HashSet::new(),
            global_read: HashSet::new(),
            global_write: HashSet::new(),
            index_map: HashMap::new(),
            next_index: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.index_map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.index_map.len()
    }

    pub fn find_code(&self, pos: CodePosition) -> Option<u32> {
        if self.code.contains(&pos) {
            for (index, breakpoint) in self {
                if let Breakpoint::Code(break_pos) = breakpoint {
                    if *break_pos == pos {
                        return Some(*index);
                    }
                }
            }
        }
        None
    }

    pub fn find_global(&self, global: u32, write: bool) -> Option<u32> {
        let found = if write {
            self.global_write.contains(&global)
        } else {
            self.global_read.contains(&global)
        };
        if found {
            for (index, breakpoint) in self {
                if let Breakpoint::Global(_, break_global) = breakpoint {
                    if *break_global == global {
                        return Some(*index);
                    }
                }
            }
        }
        None
    }

    pub fn find_memory(&self, start: u32, len: u32, write: bool) -> Option<u32> {
        let watchpoints = if write { &self.memory_write } else { &self.memory_read };
        for &addr in watchpoints {
            if start <= addr && addr < start + len {
                for (index, breakpoint) in self {
                    if let Breakpoint::Memory(_, break_addr) = breakpoint {
                        if *break_addr == addr {
                            return Some(*index);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> u32 {
        match breakpoint {
            Breakpoint::Code(position) => {
                self.code.insert(position);
            }
            Breakpoint::Memory(trigger, addr) => {
                if trigger.is_read() {
                    self.memory_read.insert(addr);
                }
                if trigger.is_write() {
                    self.memory_write.insert(addr);
                }
            }
            Breakpoint::Global(trigger, index) => {
                if trigger.is_read() {
                    self.global_read.insert(index);
                }
                if trigger.is_write() {
                    self.global_write.insert(index);
                }
            }
        };

        self.index_map.insert(self.next_index, breakpoint);
        self.next_index += 1;
        self.next_index - 1
    }

    pub fn delete_breakpoint(&mut self, index: u32) -> bool {
        if let Some(breakpoint) = self.index_map.get(&index) {
            match breakpoint {
                Breakpoint::Code(position) => {
                    self.code.remove(position);
                }
                Breakpoint::Memory(trigger, addr) => {
                    if trigger.is_read() {
                        self.memory_read.remove(addr);
                    }
                    if trigger.is_write() {
                        self.memory_write.remove(addr);
                    }
                }
                Breakpoint::Global(trigger, index) => {
                    if trigger.is_read() {
                        self.global_read.remove(index);
                    }
                    if trigger.is_write() {
                        self.global_write.remove(index);
                    }
                }
            };
            self.index_map.remove(&index);
            return true;
        }
        false
    }

    pub fn clear(&mut self) {
        self.code.clear();
        self.memory_read.clear();
        self.memory_write.clear();
        self.global_read.clear();
        self.global_write.clear();
        self.index_map.clear();
    }

    pub fn iter(&self) -> <&Self as iter::IntoIterator>::IntoIter {
        self.into_iter()
    }
}

impl<'a> iter::IntoIterator for &'a Breakpoints {
    type Item = (&'a u32, &'a Breakpoint);
    type IntoIter = <&'a HashMap<u32, Breakpoint> as iter::IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.index_map.iter()
    }
}
