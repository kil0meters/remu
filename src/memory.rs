use elf::{
    abi::{PT_GNU_STACK, PT_LOAD},
    endian::EndianParse,
    ElfBytes,
};
use log::{debug, warn};

use crate::STACK_START;

struct MemoryRange {
    start: u64,
    end: u64,
    data: Box<[u8]>,
}

impl MemoryRange {
    fn in_range(&self, idx: u64) -> bool {
        idx >= self.start && idx < self.end
    }

    fn fetch_byte(&self, idx: u64) -> u8 {
        self.data[(idx - self.start) as usize]
    }

    fn try_store_byte(&mut self, idx: u64, data: u8) -> bool {
        if self.in_range(idx) {
            self.data[(idx - self.start) as usize] = data;

            true
        } else {
            false
        }
    }
}

pub struct Memory {
    ranges: Vec<MemoryRange>,
    pub stack: Vec<u8>,
}

impl Memory {
    pub fn load_elf<T: EndianParse>(elf: ElfBytes<T>) -> Self {
        let mut ranges = Vec::new();

        let segments = elf.segments().unwrap();

        for segment in segments {
            match segment.p_type {
                PT_LOAD => {
                    let mut data = Vec::from(elf.segment_data(&segment).unwrap());

                    debug!(
                        "{}, {} (type = {})",
                        data.len(),
                        segment.p_memsz,
                        segment.p_type
                    );
                    assert!(data.len() as u64 <= segment.p_memsz);

                    data.resize(segment.p_memsz as usize, 0);

                    debug!(
                        "Mapping {} bytes onto offset {}. p_type = {}",
                        segment.p_memsz, segment.p_vaddr, segment.p_type
                    );

                    ranges.push(MemoryRange {
                        start: segment.p_vaddr,
                        end: segment.p_vaddr + data.len() as u64,
                        data: data.into(),
                    });
                }
                _ => {
                    warn!("Unknown p_type: {segment:?}");
                }
            }
        }

        Memory {
            ranges,
            stack: Vec::new(),
        }
    }

    pub fn load_u64(&mut self, index: u64) -> u64 {
        return (self.load_u32(index) as u64) | ((self.load_u32(index + 4) as u64) << 32);
    }

    pub fn load_u32(&mut self, index: u64) -> u32 {
        // let index = self.pc;
        return (self.load_byte(index) as u32)
            | ((self.load_byte(index + 1) as u32) << 8)
            | ((self.load_byte(index + 2) as u32) << 16)
            | ((self.load_byte(index + 3) as u32) << 24);
    }

    /// Has to be mutable because there's a chance loading a byte resizes the stack apparently?
    pub fn load_byte(&mut self, idx: u64) -> u8 {
        // try loading from executable mapped memory ranges
        for range in &self.ranges {
            if range.in_range(idx) {
                return range.fetch_byte(idx);
            }
        }

        // else try to load from stack
        debug!("{STACK_START:x} - {idx:x}");
        let stack_idx = STACK_START - idx;
        // debug!("Attemping to load stack index = {stack_idx}");
        if idx <= STACK_START {
            // TODO: It's possible that this extends the stack too much. IDK really
            if (stack_idx as usize) >= self.stack.len() {
                self.stack.resize(stack_idx as usize + 1, 0);
            }

            self.stack[stack_idx as usize]
        } else {
            panic!("Attempted to load from address not mapped to memory: {idx:x}");
        }
    }

    pub fn store_u64(&mut self, index: u64, data: u64) {
        self.store_u32(index, data as u32);
        self.store_u32(index + 4, (data >> 32) as u32);
    }

    pub fn store_u32(&mut self, index: u64, data: u32) {
        self.store_byte(index + 3, (data >> 24) as u8);
        self.store_byte(index + 2, (data >> 16) as u8);
        self.store_byte(index + 1, (data >> 8) as u8);
        self.store_byte(index + 0, (data) as u8);
    }

    pub fn store_byte(&mut self, idx: u64, data: u8) {
        for range in &mut self.ranges {
            if range.try_store_byte(idx, data) {
                return;
            }
        }

        // debug!("{STACK_START:x} - {idx:x}");
        let stack_idx = STACK_START - idx;
        // debug!("Attemping to load stack index = {stack_idx}");
        if idx <= STACK_START {
            // TODO: It's possible that this extends the stack too much. IDK really
            if (stack_idx as usize) >= self.stack.len() {
                self.stack.resize(stack_idx as usize + 1, 0);
            }

            self.stack[stack_idx as usize] = data;
        } else {
            panic!("Attempted to store to address not mapped to memoery: {idx:x}");
        }
    }
}
