use std::ptr;

use elf::{
    abi::{DT_NEEDED, PT_DYNAMIC, PT_INTERP, PT_LOAD, PT_PHDR},
    endian::{AnyEndian, EndianParse},
    ElfBytes,
};
use log::{debug, warn};

// comparison of hashing algorithms tested
// default (sip) => 9.01s
// xxhash        => 9.14
// nohash        => 17.68s
// fnv           => 7.81s 7.76s 7.73s
// fxhash        => 7.75s 7.88s 7.83s
pub type MemMap<K, V> = fnv::FnvHashMap<K, V>;

use crate::{
    disassembler::Disassembler,
    emulator::{FileDescriptor, STACK_START},
};

// only this constant should be changed.
// but it actually can't be changed since the linker complains :)
const PAGE_BITS: u64 = 12;

pub const PAGE_SIZE: u64 = 1 << PAGE_BITS;
pub const PAGE_MASK: u64 = (1 << PAGE_BITS) - 1;
type MemoryPage = [u8; PAGE_SIZE as usize];
const EMPTY_PAGE: MemoryPage = [0; PAGE_SIZE as usize];

pub const LD_LINUX_DATA: &'static [u8] = include_bytes!("../res/ld-linux-riscv64-lp64d.so.1");
pub const LIBC_DATA: &'static [u8] = include_bytes!("../res/libc.so.6");
pub const LIBCPP_DATA: &'static [u8] = include_bytes!("../res/libstdc++.so");
pub const LIBM_DATA: &'static [u8] = include_bytes!("../res/libm.so.6");
pub const LIBGCCS_DATA: &'static [u8] = include_bytes!("../res/libgcc_s.so.1");

pub const LIBC_FILE_DESCRIPTOR: i64 = 10;
pub const LIBCPP_FILE_DESCRIPTOR: i64 = 11;
pub const LIBM_FILE_DESCRIPTOR: i64 = 12;
pub const LIBGCCS_FILE_DESCRIPTOR: i64 = 13;

#[derive(Default, Clone)]
pub struct ProgramHeaderInfo {
    pub entry: u64,
    pub address: u64,
    pub size: u64,
    pub number: u64,
}

#[derive(Clone)]
pub struct Memory {
    // No fancy hashing algorithm here as we're not concerned about mittigating denial of service
    // attacks, and we want our program to be deterministic.
    pub pages: MemMap<u64, MemoryPage>,

    // the address to the end of the heap
    pub heap_pointer: u64,

    // address to the top of the stack, page aligned
    pub stack_pointer: u64,

    // the address of entry to the program
    pub entry: u64,

    pub program_header: ProgramHeaderInfo,

    pub disassembler: Option<Disassembler>,
}

impl Memory {
    pub fn load_elf<T: EndianParse>(elf: ElfBytes<T>, disassemble: bool) -> Self {
        let mut memory = Memory {
            entry: 0,
            program_header: ProgramHeaderInfo::default(),
            heap_pointer: 0,
            stack_pointer: STACK_START + 1,
            pages: MemMap::default(),
            disassembler: disassemble.then(Disassembler::new),
        };

        if let Some(dias) = memory.disassembler.as_mut() {
            dias.add_elf_symbols(&elf, 0);
        }

        // load dynamic libraries, if they exist
        // https://blog.k3170makan.com/2018/11/introduction-to-elf-format-part-vii.html
        // https://www.youtube.com/watch?v=Ss2e6JauS0Y
        if let Some((_dynamic_symbol_table, string_table)) = elf.dynamic_symbol_table().unwrap() {
            if let Some(dynamic) = elf.dynamic().unwrap() {
                for x in dynamic {
                    if x.d_tag == DT_NEEDED {
                        let obj = string_table.get(x.d_val() as usize).unwrap();
                        log::info!("requires shared object: {}", obj);
                    }
                }

                let ld_elf = ElfBytes::<AnyEndian>::minimal_parse(LD_LINUX_DATA).unwrap();
                log::info!("Loading dynamically linked executable.");

                let ld_offset = 0x80000000;

                memory.map_segments(ld_offset, &ld_elf);
                memory.map_segments(0x0, &elf);

                if let Some(dias) = memory.disassembler.as_mut() {
                    dias.add_elf_symbols(&ld_elf, ld_offset);
                }

                memory.entry = ld_offset + ld_elf.ehdr.e_entry;
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
                PT_LOAD | PT_PHDR | PT_DYNAMIC => {
                    let addr_start = offset + segment.p_vaddr;

                    if segment.p_type == PT_PHDR {
                        self.program_header.size = segment.p_memsz;
                        self.program_header.address = addr_start;
                        self.program_header.number = elf.ehdr.e_phnum as u64;
                        self.program_header.entry = elf.ehdr.e_entry as u64;
                    }

                    let data = elf.segment_data(&segment).unwrap();

                    assert!(data.len() as u64 <= segment.p_memsz);

                    debug!(
                        "Mapping {} bytes onto offset {:x}. p_type = {}",
                        segment.p_memsz, addr_start, segment.p_type
                    );

                    self.create_pages(addr_start, segment.p_memsz);
                    self.write_n(data, addr_start, segment.p_memsz);

                    data_end = data_end.max(offset + segment.p_vaddr + segment.p_memsz);
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

    #[cfg(test)]
    pub fn from_raw(data: &[u8]) -> Self {
        let mut memory = Memory {
            entry: 0,
            stack_pointer: STACK_START,
            heap_pointer: 0,
            pages: MemMap::default(),
            program_header: Default::default(),
        };

        memory.create_pages(0, data.len() as u64);
        memory.write_n(data, 0, data.len() as u64);

        memory
    }

    // returns the number of bytes of memory allocated
    pub fn usage(&self) -> u64 {
        self.pages.len() as u64 * PAGE_SIZE
    }

    pub fn brk(&mut self, new_end: u64) -> u64 {
        // if break point is invalid, we return the current heap pointer
        if new_end < self.heap_pointer || new_end >= self.stack_pointer {
            return self.heap_pointer;
        }

        log::info!("Changing heap by: {} bytes", new_end - self.heap_pointer);

        // the address of the last valid page on the heap
        let phys_heap_addr = self.heap_pointer & !PAGE_MASK;

        match phys_heap_addr.cmp(&new_end) {
            std::cmp::Ordering::Less => {
                for addr in (phys_heap_addr + PAGE_SIZE..=new_end).step_by(PAGE_SIZE as usize) {
                    debug_assert!(!self.pages.contains_key(&addr));
                    self.pages.insert(addr, EMPTY_PAGE);
                    self.heap_pointer += PAGE_SIZE;
                }
            }
            std::cmp::Ordering::Equal => {
                self.heap_pointer = new_end;
            }
            std::cmp::Ordering::Greater => {
                panic!("Reducing heap size is not yet supported.");
            }
        }

        new_end
    }

    // creates pages that cover the range [start_addr, start_addr+size)
    // does not overwrite
    fn create_pages(&mut self, start_addr: u64, size: u64) {
        let phys_addr = start_addr & !PAGE_MASK;
        for addr in (phys_addr..=(start_addr + size)).step_by(PAGE_SIZE as usize) {
            if !self.pages.contains_key(&addr) {
                self.pages.insert(addr, EMPTY_PAGE);
            }
        }
    }

    pub fn mmap(&mut self, addr: u64, size: u64) -> i64 {
        log::info!("MMAP REGION: 0x{:x}-0x{:x}", addr, addr + size);
        let addr = if addr == 0 {
            let region_start = 0x2000000000000000u64;

            // put region after previous region

            let mut max_addr = 0;
            for (region, _) in &self.pages {
                // not stack regions
                if *region < 0x7000000000000000 {
                    max_addr = max_addr.max(region + PAGE_SIZE);
                }
            }

            region_start.max(max_addr)
        } else {
            addr
        };

        let phys_addr = addr & !PAGE_MASK;
        log::info!("MMAP REGION: 0x{:x}-0x{:x}", addr, addr + size);

        // This overwrites the data if the addr specified happens to overlap with an existing
        // mapping. But this is the _correct_ behavior according to `man 2 mmap`
        for addr in (phys_addr..=(addr + size)).step_by(PAGE_SIZE as usize) {
            self.pages.insert(addr, EMPTY_PAGE);
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

    pub fn load_u64(&self, addr: u64) -> u64 {
        let virt_addr = addr & PAGE_MASK;
        if virt_addr < PAGE_MASK - 8 {
            // fast path
            // SAFETY: guaranteed to not cross page boundary
            unsafe { self.data_ptr_const(addr).cast::<u64>().read_unaligned() }
        } else {
            // slow path
            return (self.load_u8(addr) as u64)
                | ((self.load_u8(addr + 1) as u64) << 8)
                | ((self.load_u8(addr + 2) as u64) << 16)
                | ((self.load_u8(addr + 3) as u64) << 24)
                | ((self.load_u8(addr + 4) as u64) << 32)
                | ((self.load_u8(addr + 5) as u64) << 40)
                | ((self.load_u8(addr + 6) as u64) << 48)
                | ((self.load_u8(addr + 7) as u64) << 56);
        }
    }

    pub fn load_u32(&self, addr: u64) -> u32 {
        let virt_addr = addr & PAGE_MASK;
        if virt_addr < PAGE_MASK - 4 {
            // fast path
            // SAFETY: guaranteed to not cross page boundary
            unsafe { self.data_ptr_const(addr).cast::<u32>().read_unaligned() }
        } else {
            // slow path
            return (self.load_u8(addr) as u32)
                | ((self.load_u8(addr + 1) as u32) << 8)
                | ((self.load_u8(addr + 2) as u32) << 16)
                | ((self.load_u8(addr + 3) as u32) << 24);
        }
    }

    pub fn load_u16(&self, addr: u64) -> u16 {
        let virt_addr = addr & PAGE_MASK;
        if virt_addr < PAGE_MASK - 2 {
            // fast path
            // SAFETY: guaranteed to not cross page boundary
            unsafe { self.data_ptr_const(addr).cast::<u16>().read_unaligned() }
        } else {
            // slow path
            return (self.load_u8(addr) as u16) //.
                | ((self.load_u8(addr + 1) as u16) << 8);
        }
    }

    pub fn load_u8(&self, index: u64) -> u8 {
        // SAFETY: it's impossible for loading a byte to cross a page boundry.
        unsafe { *self.data_ptr_const(index) }
    }

    fn data_ptr_mut(&mut self, addr: u64) -> *mut u8 {
        // try loading from an page
        let phys_addr = addr & !PAGE_MASK;
        let virt_addr = addr & PAGE_MASK;

        debug_assert!(virt_addr < PAGE_SIZE);

        if let Some(page) = self.pages.get_mut(&phys_addr) {
            unsafe {
                // SAFETY: virt_addr < PAGE_SIZE
                return page.as_mut_ptr().add(virt_addr as usize);
            }
        }

        if addr <= STACK_START {
            while self.stack_pointer > addr {
                if self.stack_pointer - addr >= 0xfffff {
                    panic!("Invalid memory location: {addr:x}");
                }

                // move stack pointer down by a page
                self.stack_pointer -= PAGE_SIZE;

                debug_assert!(!self.pages.contains_key(&self.stack_pointer));
                self.pages.insert(self.stack_pointer, EMPTY_PAGE);
            }

            let page = self.pages.get_mut(&phys_addr).unwrap();
            unsafe {
                // SAFETY: virt_addr < PAGE_SIZE
                return page.as_mut_ptr().add(virt_addr as usize);
            }
        } else {
            panic!("Attempted to load to address not mapped to memory: {addr:x}");
        }
    }

    fn data_ptr_const(&self, addr: u64) -> *const u8 {
        // try loading from an page
        let phys_addr = addr & !PAGE_MASK;
        let virt_addr = addr & PAGE_MASK;

        debug_assert!(virt_addr < PAGE_SIZE);

        if let Some(page) = self.pages.get(&phys_addr) {
            unsafe {
                // SAFETY: virt_addr < PAGE_SIZE
                return page.as_ptr().add(virt_addr as usize);
            }
        } else {
            return EMPTY_PAGE.as_ptr();
        }
    }

    pub fn store_u64(&mut self, addr: u64, data: u64) {
        let virt_addr = addr & PAGE_MASK;
        if virt_addr < PAGE_MASK - 8 {
            // fast path
            // SAFETY: guaranteed to not cross page boundary
            unsafe { self.data_ptr_mut(addr).cast::<u64>().write_unaligned(data) }
        } else {
            // slow path
            self.store_u8(addr + 7, (data >> 56) as u8);
            self.store_u8(addr + 6, (data >> 48) as u8);
            self.store_u8(addr + 5, (data >> 40) as u8);
            self.store_u8(addr + 4, (data >> 32) as u8);
            self.store_u8(addr + 3, (data >> 24) as u8);
            self.store_u8(addr + 2, (data >> 16) as u8);
            self.store_u8(addr + 1, (data >> 8) as u8);
            self.store_u8(addr + 0, (data) as u8);
        }
    }

    pub fn store_u32(&mut self, addr: u64, data: u32) {
        let virt_addr = addr & PAGE_MASK;
        if virt_addr < PAGE_MASK - 4 {
            // fast path
            // SAFETY: guaranteed to not cross page boundary
            unsafe { self.data_ptr_mut(addr).cast::<u32>().write_unaligned(data) }
        } else {
            // slow path
            self.store_u8(addr + 3, (data >> 24) as u8);
            self.store_u8(addr + 2, (data >> 16) as u8);
            self.store_u8(addr + 1, (data >> 8) as u8);
            self.store_u8(addr + 0, (data) as u8);
        }
    }

    pub fn store_u16(&mut self, addr: u64, data: u16) {
        let virt_addr = addr & PAGE_MASK;
        if virt_addr < PAGE_MASK - 2 {
            // fast path
            // SAFETY: guaranteed to not cross page boundary
            unsafe { self.data_ptr_mut(addr).cast::<u16>().write_unaligned(data) }
        } else {
            // slow path
            self.store_u8(addr + 1, (data >> 8) as u8);
            self.store_u8(addr + 0, (data) as u8);
        }
    }

    pub fn store_u8(&mut self, idx: u64, data: u8) {
        // SAFETY: guaranteed to not cross page boundary
        unsafe { self.data_ptr_mut(idx).write_unaligned(data) }
    }

    pub fn write_n(&mut self, s: &[u8], addr: u64, len: u64) {
        for (i, b) in s.iter().take(len as usize).enumerate() {
            self.store_u8(addr + i as u64, *b);
        }
        for i in s.len() as u64..len {
            self.store_u8(addr + i, 0);
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

    pub fn read_file(&mut self, file_descriptor: &mut FileDescriptor, buf: u64, count: u64) -> i64 {
        let o = file_descriptor.offset as usize;
        let max = (o + count as usize).min(file_descriptor.data.len());

        let data = &file_descriptor.data[o..max];

        self.write_n(data, buf, data.len() as u64);

        data.len() as i64
    }

    pub fn hexdump(&self, mut addr: u64, length: u64) -> String {
        let mut writer = String::with_capacity(33 * length as usize);

        addr = addr & !0b111111;
        addr -= 33 * 10;

        for _ in 0..length {
            let mut line = String::with_capacity(33);
            for _ in 0..32 {
                let c = self.load_u8(addr);
                line.push(
                    if c.is_ascii_graphic() || c.is_ascii_alphabetic() || c == b' ' {
                        c
                    } else {
                        b'.'
                    } as char,
                );

                addr += 1;
            }

            line.push('\n');

            writer.push_str(&line);
        }

        writer
    }
}
