use std::collections::VecDeque;

use elf::{abi::PT_LOAD, endian::EndianParse, ElfBytes};
use log::{debug, warn};

use crate::emulator::{STACK_START, USER_STACK_OFFSET};

#[derive(Debug)]
pub struct MemoryRange {
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
    ranges: Box<[MemoryRange]>,
    pub stack: Vec<u8>,
    pub heap: Vec<u8>,

    // the address to the end of the heap
    pub heap_pointer: u64,

    // probably the best data structure is a self balancing binary search tree where node keys are
    // a range of integers (start and end of memory).
    pub mmap_regions: VecDeque<MemoryRange>,
}

impl Memory {
    pub fn load_elf<T: EndianParse>(elf: ElfBytes<T>) -> Self {
        let mut ranges = Vec::new();

        let segments = elf.segments().unwrap();
        let mut data_end = 0;

        for segment in segments {
            match segment.p_type {
                PT_LOAD => {
                    let mut data = Vec::from(elf.segment_data(&segment).unwrap());

                    assert!(data.len() as u64 <= segment.p_memsz);

                    data.resize(segment.p_memsz as usize, 0);

                    debug!(
                        "Mapping {} bytes onto offset {}. p_type = {}",
                        segment.p_memsz, segment.p_vaddr, segment.p_type
                    );

                    let range = MemoryRange {
                        start: segment.p_vaddr,
                        end: segment.p_vaddr + data.len() as u64,
                        data: data.into(),
                    };

                    data_end = data_end.max(range.end);
                    ranges.push(range);
                }
                _ => {
                    warn!("Unknown p_type: {segment:?}");
                }
            }
        }

        Memory {
            ranges: ranges.into_boxed_slice(),
            stack: Vec::new(),
            heap: Vec::new(),

            heap_pointer: data_end,
            mmap_regions: VecDeque::new(),
        }
    }

    pub fn from_raw(data: &[u8]) -> Self {
        let data = MemoryRange {
            start: 0,
            end: data.len() as u64,
            data: data.into(),
        };
        let data_end = data.end;
        Memory {
            ranges: [data].into(),
            stack: Vec::new(),
            heap: Vec::new(),
            heap_pointer: data_end,
            mmap_regions: VecDeque::new(),
        }
    }

    pub fn brk(&mut self, new_end: u64) -> u64 {
        // if break point is invalid, we return the current heap pointer
        if new_end < self.heap_pointer || new_end >= STACK_START - self.stack.len() as u64 {
            return self.heap_pointer;
        }

        self.heap.resize(
            new_end as usize - self.heap_pointer as usize + self.heap.len(),
            0,
        );

        self.heap_pointer = new_end;

        return self.heap_pointer;
    }

    pub fn mmap(&mut self, size: u64) -> u64 {
        // how do we pick and address? I don't know.

        let mut region_start = 0x2000000000000000u64;

        // put region after previous region
        let mut max_addr = 0;
        for region in &self.mmap_regions {
            max_addr = max_addr.max(region.end);
        }

        region_start = region_start.max(max_addr);

        let region = MemoryRange {
            start: region_start,
            end: region_start + size,
            data: vec![0; size as usize].into_boxed_slice(),
        };

        println!("MMAP REGION AT:  {}", region.start);
        println!("MMAP REGION END: {}", region.end);

        self.mmap_regions.push_back(region);

        region_start
    }

    pub fn munmap(&mut self, ptr: u64) -> u64 {
        let index = self.mmap_regions.iter().position(|elm| elm.start == ptr);

        if let Some(index) = index {
            self.mmap_regions.swap_remove_back(index);
            return 0;
        } else {
            return -1 as i64 as u64;
        }
    }

    pub fn load_u64(&mut self, index: u64) -> u64 {
        return (self.load_u32(index) as u64) | ((self.load_u32(index + 4) as u64) << 32);
    }

    pub fn load_u32(&mut self, index: u64) -> u32 {
        return (self.load_u8(index) as u32)
            | ((self.load_u8(index + 1) as u32) << 8)
            | ((self.load_u8(index + 2) as u32) << 16)
            | ((self.load_u8(index + 3) as u32) << 24);
    }

    pub fn load_u16(&mut self, index: u64) -> u16 {
        return (self.load_u8(index) as u16) //.
            | ((self.load_u8(index + 1) as u16) << 8);
    }

    /// Has to be mutable because there's a chance loading a byte resizes the stack apparently?
    pub fn load_u8(&mut self, idx: u64) -> u8 {
        // try loading from executable mapped memory ranges
        for range in self.ranges.iter() {
            if range.in_range(idx) {
                return range.fetch_byte(idx);
            }
        }

        // try loading from dynamic mmap regions
        for range in self.mmap_regions.iter() {
            if range.in_range(idx) {
                return range.fetch_byte(idx);
            }
        }

        let heap_start = self.heap_pointer - self.heap.len() as u64;
        if idx >= heap_start && idx < self.heap_pointer {
            let heap_idx = idx - heap_start;

            self.heap[heap_idx as usize]
        } else if idx <= STACK_START {
            let stack_idx = STACK_START - idx;

            // TODO: It's possible that this extends the stack too much. IDK really
            if (stack_idx as usize) >= self.stack.len() {
                if stack_idx as usize - self.stack.len() < 0xffffff {
                    self.stack.resize(stack_idx as usize + 1, 0);
                }
            }

            self.stack[stack_idx as usize]
        } else {
            panic!("Attempted to load to address not mapped to memoery: {idx:x}");
        }
    }

    pub fn store_u64(&mut self, index: u64, data: u64) {
        self.store_u32(index, data as u32);
        self.store_u32(index + 4, (data >> 32) as u32);
    }

    pub fn store_u32(&mut self, index: u64, data: u32) {
        self.store_u8(index + 3, (data >> 24) as u8);
        self.store_u8(index + 2, (data >> 16) as u8);
        self.store_u8(index + 1, (data >> 8) as u8);
        self.store_u8(index + 0, (data) as u8);
    }

    pub fn store_u16(&mut self, index: u64, data: u16) {
        self.store_u8(index + 1, (data >> 8) as u8);
        self.store_u8(index + 0, (data) as u8);
    }

    pub fn store_u8(&mut self, idx: u64, data: u8) {
        for range in self.ranges.iter_mut() {
            if range.try_store_byte(idx, data) {
                return;
            }
        }

        for range in self.mmap_regions.iter_mut() {
            if range.in_range(idx) {
                if range.try_store_byte(idx, data) {
                    return;
                }
            }
        }

        let heap_start = self.heap_pointer - self.heap.len() as u64;
        if idx >= heap_start && idx < self.heap_pointer {
            // debug!(
            //     "store byte: {:x} - ({:x} - {:x})",
            //     idx,
            //     self.heap_pointer,
            //     self.heap.len()
            // );

            let heap_idx = idx - heap_start;

            self.heap[heap_idx as usize] = data;
        } else if idx <= STACK_START {
            let stack_idx = STACK_START - idx;

            // TODO: It's possible that this extends the stack too much. IDK really
            if (stack_idx as usize) >= self.stack.len() {
                if stack_idx as usize - self.stack.len() < 0xffffff {
                    self.stack.resize(stack_idx as usize + 1, 0);
                } else {
                    return;
                }
            }

            self.stack[stack_idx as usize] = data;
        } else {
            panic!("Attempted to store to address not mapped to memoery: {idx:x}");
        }
    }

    pub fn write_string_n(&mut self, s: &[u8], addr: u64, len: u64) {
        for i in 0..(len.min(s.len() as u64)) {
            println!("Writing to: {:x}", addr + i);
            self.store_u8(addr + i, s[i as usize]);
        }
    }

    // super unsafe, probably requires null termination
    pub fn read_string(&mut self, mut addr: u64) -> String {
        let mut data = Vec::new();
        // read bytes until we get null
        loop {
            let c = self.load_u8(addr);
            addr += 1;

            if c == 0 {
                break;
            }

            data.push(c);
        }

        let s = String::from_utf8_lossy(&data);
        s.into()
    }
}
