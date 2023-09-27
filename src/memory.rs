use std::collections::{HashMap, VecDeque};

use elf::{
    abi::{DT_NEEDED, DT_SYMBOLIC, PT_DYNAMIC, PT_INTERP, PT_LOAD, PT_PHDR, PT_TLS},
    endian::{AnyEndian, EndianParse},
    segment::SegmentTable,
    ElfBytes,
};
use log::{debug, warn};

use crate::emulator::{FileDescriptor, STACK_START};

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

pub const PAGESIZE: u64 = 1 << 12;
pub const PAGE_MASK: u64 = (1 << 12) - 1;
type MemoryPage = [u8; PAGESIZE as usize];

pub const LD_LINUX_DATA: &'static [u8] = include_bytes!("../res/ld-linux-riscv64-lp64d.so.1");
pub const LIBC_DATA: &'static [u8] = include_bytes!("../res/libc.so.6");
pub const LIBCPP_DATA: &'static [u8] = include_bytes!("../res/libstdc++.so");
pub const LIBM_DATA: &'static [u8] = include_bytes!("../res/libm.so.6");
pub const LIBGCCS_DATA: &'static [u8] = include_bytes!("../res/libgcc_s.so.1");

pub const LIBC_FILE_DESCRIPTOR: i64 = 10;
pub const LIBCPP_FILE_DESCRIPTOR: i64 = 11;
pub const LIBM_FILE_DESCRIPTOR: i64 = 12;
pub const LIBGCCS_FILE_DESCRIPTOR: i64 = 13;

#[derive(Default)]
pub struct ProgramHeaderInfo {
    pub entry: u64,
    pub address: u64,
    pub size: u64,
    pub number: u64,
}

pub struct Memory {
    pub ranges: Vec<MemoryRange>,
    pub stack: Vec<u8>,
    pub heap: Vec<u8>,

    // the address of entry to the program
    pub entry: u64,

    pub program_header: ProgramHeaderInfo,

    // the address to the end of the heap
    pub heap_pointer: u64,

    // probably the best data structure is a self balancing binary search tree where node keys are
    // a range of integers (start and end of memory).
    pub mmap_pages: HashMap<u64, MemoryPage>,
}

impl Memory {
    pub fn load_elf<T: EndianParse>(elf: ElfBytes<T>) -> Self {
        let mut memory = Memory {
            ranges: Vec::new(),
            stack: vec![0; 512],
            heap: Vec::new(),
            entry: 0,
            program_header: ProgramHeaderInfo::default(),
            heap_pointer: 0,
            mmap_pages: HashMap::new(),
        };

        // load dynamic libraries, if they exist
        // https://blog.k3170makan.com/2018/11/introduction-to-elf-format-part-vii.html
        // https://www.youtube.com/watch?v=Ss2e6JauS0Y
        if let Some((dynamic_symbol_table, string_table)) = elf.dynamic_symbol_table().unwrap() {
            if let Some(dynamic) = elf.dynamic().unwrap() {
                for x in dynamic {
                    if x.d_tag == DT_NEEDED {
                        let obj = string_table.get(x.d_val() as usize).unwrap();
                        println!("dynamic links with: {}", obj);
                    }
                }

                let rela_header = elf.section_header_by_name(".rela.plt").unwrap().unwrap();
                let rela_data = elf.section_data_as_relas(&rela_header).unwrap();

                let mut symbols_to_link = Vec::new();
                for rel in rela_data {
                    let symbol = dynamic_symbol_table.get(rel.r_sym as usize).unwrap();
                    let symbol_name = string_table.get(symbol.st_name as usize).unwrap();

                    symbols_to_link.push((symbol_name, rel.r_offset));
                }

                // let libc_elf = ElfBytes::<AnyEndian>::minimal_parse(LIBC_DATA).unwrap();
                // let (libc_symbol_table, libc_string_table) =
                //     libc_elf.dynamic_symbol_table().unwrap().unwrap();
                //
                // println!("{:?}", symbols_to_link);
                //
                // for (symbol_name, offset) in symbols_to_link {
                //     for libc_symbol in libc_symbol_table.iter() {
                //         let libc_symbol_name =
                //             libc_string_table.get(libc_symbol.st_name as usize).unwrap();
                //
                //         if libc_symbol_name == symbol_name {
                //             log::info!("dynamically linking {symbol_name}({offset:x}) => libc::{libc_symbol_name}({:x})", libc_symbol.st_value);
                //
                //             memory.store_u64(offset, libc_symbol.st_value);
                //         }
                //     }
                // }

                let ld_elf = ElfBytes::<AnyEndian>::minimal_parse(LD_LINUX_DATA).unwrap();
                // let (ld_symbol_table, ld_string_table) =
                //     ld_elf.dynamic_symbol_table().unwrap().unwrap();
                let (ld_symbol_table, ld_string_table) = ld_elf.symbol_table().unwrap().unwrap();

                let dl_runtime_resolve_sym = ld_symbol_table
                    .iter()
                    .find(|ld_symbol| {
                        let ld_symbol_name = ld_string_table.get(ld_symbol.st_name as usize);
                        if let Ok("_dl_runtime_resolve") = ld_symbol_name {
                            true
                        } else {
                            false
                        }
                    })
                    .expect("Failed to find _dl_runtime_resolve in dynamic linker");

                log::info!("Loading dynamically linked executable.");

                let ld_offset = 0x80000000;

                memory.map_segments(ld_offset, &ld_elf);
                memory.map_segments(0x0, &elf);

                memory.entry = ld_offset + ld_elf.ehdr.e_entry;

                // TODO: make this work dynamically
                memory.store_u64(8200, ld_offset + dl_runtime_resolve_sym.st_value);
            }
        } else {
            log::info!("Loading statically linked executable.");
            memory.map_segments(0, &elf);
            memory.entry = elf.ehdr.e_entry;
        }

        memory
    }

    fn map_segments<'data, E: EndianParse>(&mut self, offset: u64, elf: &ElfBytes<'data, E>) {
        let mut data_end = self.heap_pointer.max(offset);

        let segments = elf.segments().unwrap();
        for segment in segments {
            match segment.p_type {
                PT_LOAD | PT_PHDR | PT_TLS | PT_DYNAMIC => {
                    if segment.p_type == PT_PHDR {
                        self.program_header.size = segment.p_memsz;
                        self.program_header.address = offset + segment.p_vaddr;
                        self.program_header.number = elf.ehdr.e_phnum as u64;
                        self.program_header.entry = elf.ehdr.e_entry as u64;
                    }

                    let mut data = Vec::from(elf.segment_data(&segment).unwrap());
                    assert!(data.len() as u64 <= segment.p_memsz);

                    data.resize(segment.p_memsz as usize, 0);

                    debug!(
                        "Mapping {} bytes onto offset {:x}. p_type = {}",
                        segment.p_memsz,
                        offset + segment.p_vaddr,
                        segment.p_type
                    );

                    let range = MemoryRange {
                        start: offset + segment.p_vaddr,
                        end: offset + segment.p_vaddr + data.len() as u64,
                        data: data.into(),
                    };

                    data_end = data_end.max(range.end);
                    self.ranges.push(range);
                }
                PT_INTERP => {
                    log::debug!("interp: {segment:x?}");
                }
                _ => {
                    warn!("Unknown p_type: {segment:x?}");
                }
            }
        }

        self.heap_pointer = data_end;
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
            entry: 0,
            heap_pointer: data_end,
            mmap_pages: HashMap::new(),
            program_header: Default::default(),
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

    pub fn mmap(&mut self, addr: u64, size: u64) -> i64 {
        // if size is not multiple of PAGESIZE, error
        if addr % PAGESIZE != 0 {
            return -1;
        }

        let addr = if addr == 0 {
            let region_start = 0x2000000000000000u64;

            // put region after previous region
            let mut max_addr = 0;
            for (region, _) in &self.mmap_pages {
                max_addr = max_addr.max(region + PAGESIZE);
            }

            region_start.max(max_addr)
        } else {
            // TODO, ensure not overlapping and shit.
            addr
        };

        log::info!("MMAP REGION: 0x{:x}-0x{:x}", addr, addr + size);

        // This overwrites the data if the addr specified happens to overlap with an existing
        // mapping. But this is the _correct_ behavior according to `man 2 mmap`
        for addr in (addr..(addr + size)).step_by(PAGESIZE as usize) {
            self.mmap_pages.insert(addr, [0; 4096]);
        }

        addr as i64
    }

    pub fn mmap_file(
        &mut self,
        descriptor: &FileDescriptor,
        addr: u64,
        offset: u64,
        len: u64,
    ) -> i64 {
        // TODO: assert offset is multiple of pagesize
        let data = &descriptor.data[(offset as usize)..(offset as usize + len as usize)];

        assert_eq!(data.len() as u64, len);

        let addr_start = self.mmap(addr, data.len() as u64);

        if addr_start >= 0 {
            self.write_n(data, addr_start as u64, len);
        }

        addr_start
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

    pub fn load_u8(&mut self, index: u64) -> u8 {
        unsafe { *self.data_ptr(index) }
    }

    unsafe fn data_ptr(&mut self, idx: u64) -> *mut u8 {
        // try loading from executable mapped memory ranges
        for range in self.ranges.iter_mut() {
            if range.in_range(idx) {
                return range.fetch_byte(idx);
            }
        }

        // try loading from a mmap page
        let phys_addr = idx & !PAGE_MASK;
        if let Some(page) = self.mmap_pages.get_mut(&phys_addr) {
            let virt_addr = idx & PAGE_MASK;
            return page.as_mut_ptr().add(virt_addr as usize);
        }

        let heap_start = self.heap_pointer - self.heap.len() as u64;
        if idx >= heap_start && idx < self.heap_pointer {
            let heap_idx = idx - heap_start;

            self.heap.as_mut_ptr().add(heap_idx as usize)
        } else if idx <= STACK_START {
            let mut stack_end = STACK_START - self.stack.len() as u64;

            while stack_end > idx {
                if stack_end - idx >= 0xfffff {
                    panic!("Invalid memory location: {idx:x}");
                }

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

    pub fn write_n(&mut self, s: &[u8], addr: u64, len: u64) {
        for (i, b) in s.iter().take(len as usize).enumerate() {
            self.store_u8(addr + i as u64, *b);
        }
    }

    pub fn read_string_n(&mut self, mut addr: u64, len: u64) -> String {
        let mut data = Vec::new();
        // read bytes until we get null
        for _ in 0..len {
            let c = self.load_u8(addr);
            addr += 1;

            if c == b'\0' {
                break;
            }

            data.push(c);
        }

        let s = String::from_utf8_lossy(&data);
        s.into()
    }

    // super unsafe, probably requires null termination
    // pub unsafe fn read_string(&mut self, mut addr: u64) -> String {
    //     let mut data = Vec::new();
    //     // read bytes until we get null
    //     loop {
    //         let c = self.load_u8(addr);
    //         addr += 1;
    //
    //         if c == 0 {
    //             break;
    //         }
    //
    //         data.push(c);
    //     }
    //
    //     let s = String::from_utf8_lossy(&data);
    //     s.into()
    // }

    pub fn read_file(&mut self, file_descriptor: &mut FileDescriptor, buf: u64, count: u64) -> i64 {
        let o = file_descriptor.offset as usize;
        let max = (o + count as usize).min(file_descriptor.data.len());

        let data = &file_descriptor.data[o..max];

        self.write_n(data, buf, count);

        if data.len() as u64 != count {
            0
        } else {
            data.len() as i64
        }
    }
}
