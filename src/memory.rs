use std::collections::VecDeque;

use elf::{abi::PT_LOAD, endian::EndianParse, ElfBytes};
use log::{debug, warn};

use crate::emulator::STACK_START;

#[derive(Debug)]
pub struct MemoryRange {
    pub start: u64,
    pub end: u64,
    pub data: Box<[u8]>,
}

impl MemoryRange {
    fn in_range(&self, idx: u64) -> bool {
        idx >= self.start && idx < self.end
    }

    // SAFETY: valid if in range
    unsafe fn fetch_byte(&mut self, idx: u64) -> *mut u8 {
        self.data.as_mut_ptr().add((idx - self.start) as usize)
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
            stack: vec![0; 512],
            heap: Vec::new(),

            heap_pointer: data_end,
            mmap_regions: VecDeque::new(),
        }
    }

    pub fn get_text_range(&self) -> (u64, u64) {
        (self.ranges[0].start, self.ranges[0].end)
    }

    #[cfg(test)]
    pub fn from_raw(data: &[u8]) -> Self {
        let data = MemoryRange {
            start: 0,
            end: data.len() as u64,
            data: data.into(),
        };
        let data_end = data.end;
        Memory {
            ranges: [data].into(),
            stack: vec![0; 512],
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

    // pub fn munmap(&mut self, ptr: u64) -> u64 {
    //     let index = self.mmap_regions.iter().position(|elm| elm.start == ptr);
    //
    //     if let Some(index) = index {
    //         self.mmap_regions.swap_remove_back(index);
    //         return 0;
    //     } else {
    //         return -1 as i64 as u64;
    //     }
    // }

    pub fn load_u64(&mut self, index: u64) -> u64 {
        unsafe { self.data_ptr(index).cast::<u64>().read_unaligned() }
    }

    pub fn load_u32(&mut self, index: u64) -> u32 {
        unsafe { self.data_ptr(index).cast::<u32>().read_unaligned() }
    }

    pub fn load_u16(&mut self, index: u64) -> u16 {
        unsafe { self.data_ptr(index).cast::<u16>().read_unaligned() }
    }

    unsafe fn data_ptr(&mut self, idx: u64) -> *mut u8 {
        // try loading from executable mapped memory ranges
        for range in self.ranges.iter_mut() {
            if range.in_range(idx) {
                return range.fetch_byte(idx);
            }
        }

        // try loading from dynamic mmap regions
        for range in self.mmap_regions.iter_mut() {
            if range.in_range(idx) {
                return range.fetch_byte(idx);
            }
        }

        let heap_start = self.heap_pointer - self.heap.len() as u64;
        if idx >= heap_start && idx < self.heap_pointer {
            let heap_idx = idx - heap_start;

            self.heap.as_mut_ptr().add(heap_idx as usize)
        } else if idx <= STACK_START {
            let mut stack_end = STACK_START - self.stack.len() as u64;

            while stack_end > idx {
                // resize and shift
                // manual vec implementation here
                self.stack.extend_from_within(0..self.stack.len());

                stack_end = STACK_START - self.stack.len() as u64;
            }

            self.stack.as_mut_ptr().add((idx - stack_end) as usize)
        } else {
            panic!("Attempted to load to address not mapped to memoery: {idx:x}");
        }
    }

    /// Has to be mutable because there's a chance loading a byte resizes the stack apparently?
    pub fn load_u8(&mut self, idx: u64) -> u8 {
        unsafe { *self.data_ptr(idx) }
    }

    pub fn store_u64(&mut self, idx: u64, data: u64) {
        unsafe { self.data_ptr(idx).cast::<u64>().write_unaligned(data) }
    }

    pub fn store_u32(&mut self, idx: u64, data: u32) {
        unsafe { self.data_ptr(idx).cast::<u32>().write_unaligned(data) }
    }

    pub fn store_u16(&mut self, idx: u64, data: u16) {
        unsafe { self.data_ptr(idx).cast::<u16>().write_unaligned(data) }
    }

    pub fn store_u8(&mut self, idx: u64, data: u8) {
        unsafe { self.data_ptr(idx).write_unaligned(data) }
    }

    pub fn write_string_n(&mut self, s: &[u8], addr: u64, len: u64) {
        for i in 0..(len.min(s.len() as u64)) {
            self.store_u8(addr + i, s[i as usize]);
        }
    }

    pub fn read_string_n(&mut self, mut addr: u64, len: u64) -> String {
        let mut data = Vec::new();
        // read bytes until we get null
        for _ in 0..len {
            let c = self.load_u8(addr);
            addr += 1;
            data.push(c);
        }

        let s = String::from_utf8_lossy(&data);
        s.into()
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
