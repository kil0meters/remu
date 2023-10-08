use std::{
    mem,
    ops::{Index, IndexMut},
};

use elf::{
    abi::{DT_NEEDED, PT_DYNAMIC, PT_INTERP, PT_LOAD, PT_PHDR},
    endian::{AnyEndian, EndianParse},
    ElfBytes,
};
use log::{debug, warn};

use crate::{
    disassembler::Disassembler,
    emulator::{FileDescriptor, STACK_START},
    error::RVError,
};

const PAGE_BITS: u64 = 12;
pub const PAGE_SIZE: u64 = 1 << PAGE_BITS;
pub const PAGE_MASK: u64 = (1 << PAGE_BITS) - 1;

pub const LD_LINUX_DATA: &'static [u8] = include_bytes!("../../res/ld-linux-riscv64-lp64d.so.1");
pub const LIBC_DATA: &'static [u8] = include_bytes!("../../res/libc.so.6");
pub const LIBCPP_DATA: &'static [u8] = include_bytes!("../../res/libstdc++.so");
pub const LIBM_DATA: &'static [u8] = include_bytes!("../../res/libm.so.6");
pub const LIBGCCS_DATA: &'static [u8] = include_bytes!("../../res/libgcc_s.so.1");

pub const LIBC_FILE_DESCRIPTOR: i64 = 10;
pub const LIBCPP_FILE_DESCRIPTOR: i64 = 11;
pub const LIBM_FILE_DESCRIPTOR: i64 = 12;
pub const LIBGCCS_FILE_DESCRIPTOR: i64 = 13;

#[derive(Clone, Copy, PartialEq, Eq)]
struct HeapIndex(u8);

impl Index<HeapIndex> for [Vec<u8>] {
    type Output = Vec<u8>;
    fn index(&self, index: HeapIndex) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl IndexMut<HeapIndex> for [Vec<u8>] {
    fn index_mut(&mut self, index: HeapIndex) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

#[derive(Default, Clone)]
pub struct ProgramHeaderInfo {
    pub entry: u64,
    pub address: u64,
    pub size: u64,
    pub number: u64,
}

#[derive(Clone)]
pub struct Memory {
    // buffer 0:     program data
    // buffer 1:     heap
    // buffer 2:     dynamic linker (if available)
    // buffer 3-245: mmap regions
    // buffer 255:   stack
    buffers: [Vec<u8>; 256],

    // the address of entry to the program
    pub entry: u64,

    pub program_header: ProgramHeaderInfo,

    pub disassembler: Disassembler,

    // the number of times mmap has been called
    pub mmap_count: u64,
}

impl Memory {
    pub fn load_elf<T: EndianParse>(elf: ElfBytes<T>) -> Self {
        let mut memory = Memory {
            buffers: vec![vec![]; 256].try_into().expect("static"),
            entry: 0,
            program_header: ProgramHeaderInfo::default(),
            mmap_count: 3,
            disassembler: Disassembler::new(),
        };

        // add an initial page to the stack
        memory.buffers[255].resize(0x1000, 0);

        memory.disassembler.add_elf_symbols(&elf, 0);

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

                let ld_offset = memory.heap_end(HeapIndex(2));

                memory.map_segments(ld_offset, &ld_elf);
                memory.map_segments(0x0, &elf);

                memory.disassembler.add_elf_symbols(&ld_elf, ld_offset);

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

                    // grows a heap to contain address, if necessary
                    let index = Self::heap_index(addr_start + segment.p_memsz);
                    if self.heap_end(index) < addr_start + (segment.p_memsz | PAGE_MASK) {
                        self.grow_heap(addr_start + (segment.p_memsz | PAGE_MASK));
                    }

                    self.write_n(data, addr_start, segment.p_memsz)
                        .expect("Failed to load executable into memory");
                }
                PT_INTERP => {
                    log::debug!("interp: {segment:x?}");
                }
                _ => {
                    warn!("Unknown p_type: {segment:x?}");
                }
            }
        }
    }

    #[cfg(test)]
    pub fn from_raw(data: &[u8]) -> Self {
        let mut memory = Memory {
            entry: 0,
            mmap_count: 0,
            disassembler: Disassembler::new(),
            program_header: Default::default(),
            buffers: vec![vec![]; 256].try_into().expect("static"),
        };

        memory.buffers[255].resize(0x1000, 0);

        memory.grow_heap(data.len() as u64);
        memory
            .write_n(data, 0, data.len() as u64)
            .expect("Failed to write data for test");

        memory
    }

    // returns the number of bytes of memory allocated
    pub fn usage(&self) -> u64 {
        return 0;

        // this is way too slow, should be fixed
        // let mut total = 0;
        // for buffer in &self.buffers {
        //     total += buffer.len();
        // }
        // return total as u64;
    }

    pub fn brk(&mut self, new_end: u64) -> u64 {
        // ensure address is within heap bounds
        let val = new_end >> 56;
        if val == 1 {
            self.grow_heap(new_end);
        }

        return 0x0100000000000000 + self.buffers[1].len() as u64;
    }

    // sets a heap size to new_end
    fn grow_heap(&mut self, new_addr: u64) {
        let heap_index = Self::heap_index(new_addr);
        let heap_size = new_addr & 0x00FFFFFFFFFFFFFF;
        match heap_index.0 {
            0..=254 => {
                log::debug!("Growing heap {} to size = {:x}", heap_index.0, heap_size);
                self.buffers[heap_index].resize(heap_size as usize, 0);
                log::debug!("heap size: {:x}", self.buffers[heap_index].len());
            }
            255 => {
                unimplemented!();
            }
        }
    }

    /// gets the heap index of a given address
    fn heap_index(addr: u64) -> HeapIndex {
        HeapIndex((addr >> 56) as u8)
    }

    /// gets the index into the heap
    fn heap_addr(addr: u64) -> u64 {
        0x00FFFFFFFFFFFFFF & addr
    }

    /// returns the end of a heap with a given index
    fn heap_end(&self, index: HeapIndex) -> u64 {
        0x0100000000000000 * index.0 as u64 + self.buffers[index].len() as u64
    }

    pub fn mmap(&mut self, addr: u64, size: u64) -> i64 {
        log::info!("MMAP REGION: 0x{:x}-0x{:x}", addr, addr + size);

        // we can only have a maximum of 254 memory mapped regions
        if self.mmap_count > 254 {
            return -1;
        }

        // if the user does not ask for an address, we start a new buffer
        if addr == 0 {
            let addr = 0x0100000000000000 * self.mmap_count;
            self.mmap_count += 1;

            // take note to align to page boundary
            self.grow_heap(addr + (size | PAGE_MASK));

            addr as i64
        }
        // if the user asks for a specific block of memory
        else {
            let heap_index = Self::heap_index(addr);

            // only grow the heap of the memory region extends past the current heap end
            if self.heap_end(heap_index) < addr + (size | PAGE_MASK) {
                self.grow_heap(addr + (size | PAGE_MASK));
            }

            // This overwrites the data if the addr specified happens to overlap with an existing
            // mapping. But this is the _correct_ behavior according to `man 2 mmap`
            for i in addr..(addr + (size | PAGE_MASK)) {
                self.store(i, 0u8).expect("This shoudl not fail");
            }

            addr as i64
        }
    }

    pub fn mmap_file(
        &mut self,
        descriptor: &FileDescriptor,
        addr: u64,
        offset: u64,
        len: u64,
    ) -> Result<i64, RVError> {
        // TODO: assert offset is multiple of pagesize
        let data = &descriptor.data[(offset as usize)..(offset as usize + len as usize)];

        debug_assert_eq!(data.len() as u64, len);

        let addr_start = self.mmap(addr, data.len() as u64);

        if addr_start >= 0 {
            self.write_n(data, addr_start as u64, len)?;
        }

        Ok(addr_start)
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

    pub fn store<T>(&mut self, addr: u64, data: T) -> Result<(), RVError> {
        let heap_index = Self::heap_index(addr);
        let heap_addr = Self::heap_addr(addr);

        let buffer = &mut self.buffers[heap_index];
        // log::debug!(
        //     "storing {} bytes to {addr:x}, bufsize={:x}",
        //     mem::size_of::<T>(),
        //     buffer.len()
        // );
        // log::debug!(
        //     "{:x} <= {:x}",
        //     heap_addr + mem::size_of::<T>() as u64,
        //     buffer.len()
        // );

        if heap_index == HeapIndex(255) {
            let mut stack_end = STACK_START - buffer.len() as u64;

            while stack_end > addr {
                // don't resize of bigger than a page
                if stack_end - addr > 0x1000 {
                    return Err(RVError::SegmentationFault);
                }

                // resize and shift
                // manual vec implementation here
                buffer.extend_from_within(0..buffer.len());

                stack_end = STACK_START - buffer.len() as u64;
            }

            unsafe {
                // SAFETY: if we got to this point the stack has been resized to the proper size already
                buffer
                    .as_mut_ptr()
                    .add((addr - stack_end) as usize)
                    .cast::<T>()
                    .write_unaligned(data);
            }

            Ok(())
        } else if heap_addr as usize + mem::size_of::<T>() <= buffer.len() {
            unsafe {
                // SAFETY: Write is guaranteed to be within buffer bounds
                buffer
                    .as_mut_ptr()
                    .add(heap_addr as usize)
                    .cast::<T>()
                    .write_unaligned(data);

                Ok(())
            }
        } else {
            return Err(RVError::SegmentationFault);
        }
    }

    pub fn load<T>(&self, addr: u64) -> Result<T, RVError> {
        let heap_index = Self::heap_index(addr);
        let heap_addr = Self::heap_addr(addr);

        let buffer = &self.buffers[heap_index];

        if heap_index == HeapIndex(255) {
            let stack_end = STACK_START - buffer.len() as u64;

            if addr > stack_end {
                // SAFETY: guaranteed to be on stack
                unsafe {
                    return Ok(buffer
                        .as_ptr()
                        .add((addr - stack_end) as usize)
                        .cast::<T>()
                        .read_unaligned());
                }
            } else {
                return Err(RVError::SegmentationFault);
            }
        } else if heap_addr as usize + mem::size_of::<T>() <= buffer.len() {
            unsafe {
                // SAFETY: Read is guaranteed to be within buffer bounds
                return Ok(buffer
                    .as_ptr()
                    .add(heap_addr as usize)
                    .cast::<T>()
                    .read_unaligned());
            }
        } else {
            return Err(RVError::SegmentationFault);
        }
    }

    pub fn write_n(&mut self, s: &[u8], addr: u64, len: u64) -> Result<(), RVError> {
        // TODO: use slice copying method to make this more efficient

        for (i, b) in s.iter().take(len as usize).enumerate() {
            self.store::<u8>(addr + i as u64, *b)?;
        }

        for i in s.len() as u64..len {
            // println!("store: {:x} going to {:x}", addr + i, addr + len);
            self.store::<u8>(addr + i, 0)?;
        }

        Ok(())
    }

    pub fn read_string_n(&mut self, mut addr: u64, len: u64) -> Result<String, RVError> {
        let mut data = Vec::new();
        // read bytes until we get null
        for _ in 0..len {
            let c = self.load(addr)?;
            addr += 1;

            if c == b'\0' {
                break;
            }

            data.push(c);
        }

        let s = String::from_utf8_lossy(&data);
        Ok(s.into())
    }

    pub fn read_file(
        &mut self,
        file_descriptor: &mut FileDescriptor,
        buf: u64,
        count: u64,
    ) -> Result<i64, RVError> {
        let o = file_descriptor.offset as usize;
        let max = (o + count as usize).min(file_descriptor.data.len());

        let data = &file_descriptor.data[o..max];

        self.write_n(data, buf, data.len() as u64)?;

        Ok(data.len() as i64)
    }

    pub fn hexdump(&self, mut addr: u64, length: u64) -> String {
        let mut writer = String::with_capacity(33 * length as usize);

        addr = addr & !0b111111;
        addr -= addr.saturating_sub(33 * 10);

        for _ in 0..length {
            let mut line = String::with_capacity(33);
            for _ in 0..32 {
                let c: u8 = self.load(addr).unwrap_or(0);
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
