use std::collections::HashMap;

use elf::{
    abi::{STT_FILE, STT_FUNC, STT_NOTYPE},
    endian::EndianParse,
    ElfBytes,
};

use crate::{instruction::Inst, memory::Memory};

#[derive(Clone)]
pub struct Disassembler {
    symbols: Vec<(u64, String)>,
}

impl Disassembler {
    pub fn new() -> Disassembler {
        Disassembler {
            symbols: Vec::default(),
        }
    }

    // offset: the address offset in memory
    pub fn add_elf_symbols<T: EndianParse>(&mut self, elf: &ElfBytes<T>, offset: u64) {
        // add symbols
        let (symbol_table, string_table) = elf.symbol_table().unwrap().unwrap();

        for symbol in symbol_table.iter() {
            let symtype = symbol.st_symtype();
            if symtype == STT_FUNC || symtype == STT_NOTYPE {
                let symbol_name = string_table.get(symbol.st_name as usize).unwrap();
                self.symbols
                    .push((symbol.st_value + offset, symbol_name.to_string()));
            }
        }

        // also push .text and .plt start sections
        if let Some(plt_header) = elf.section_header_by_name(".plt").unwrap() {
            self.symbols
                .push((plt_header.sh_addr + offset, ".plt".to_string()));
        }

        // let text_header = elf
        //     .section_header_by_name(".text")
        //     .unwrap()
        //     .expect("no .text section");
        // self.symbols
        //     .push((text_header.sh_addr + offset, ".text".to_string()));

        self.symbols.sort_unstable_by_key(|a| a.0);
    }

    pub fn disassemble_elf<T: EndianParse>(elf: &ElfBytes<T>) -> String {
        let mut dias = Disassembler::new();
        dias.add_elf_symbols(elf, 0);

        let mut text_regions = Vec::new();
        let mut instructions = HashMap::new();

        for section_name in [".text", ".plt"] {
            // add instructions
            if let Some(section_header) = elf.section_header_by_name(section_name).unwrap() {
                let start = section_header.sh_addr;
                let end = start + section_header.sh_size;
                text_regions.push((start, end));

                let (text_data, _) = elf
                    .section_data(&section_header)
                    .expect("Failed to get text data");

                // walk through until we reach the end
                let mut pc = 0;
                while pc < section_header.sh_size as usize {
                    // should be fine, right?
                    let inst_data = (text_data[pc] as u32)
                        | ((text_data[pc + 1] as u32) << 8)
                        | ((*text_data.get(pc + 2).unwrap_or(&0) as u32) << 16)
                        | ((*text_data.get(pc + 3).unwrap_or(&0) as u32) << 24);

                    let (inst, step) = Inst::decode(inst_data);

                    instructions.insert(pc as u64 + start, (inst, step));
                    pc += step as usize;
                }
            }
        }

        let mut writer = String::new();

        for (start, end) in &text_regions {
            let mut pc = *start;
            while pc < *end {
                let (inst, step) = instructions.get(&pc).unwrap();

                writer.push_str(&format!("{}\n", dias.disassemble_inst(*inst, pc)));

                pc += *step as u64;
            }

            writer.push_str("\n");
        }

        writer
    }

    /// disassembles ~n instructions around pc
    pub fn disassemble_pc_relative(&self, memory: &Memory, start_pc: u64, n: u64) -> String {
        let mut writer = String::new();

        let mut pc = start_pc - 4 * n;

        let mut count_after = 0;

        while count_after < n {
            let inst_data = memory.load(pc).unwrap_or(0);
            let (inst, size) = Inst::decode(inst_data);

            writer.push_str(&format!("{}\n", self.disassemble_inst(inst, pc)));

            pc += size as u64;

            if pc > start_pc {
                count_after += 1;
            }
        }

        writer
    }

    pub fn get_symbol_at_addr(&self, addr: u64) -> Option<String> {
        self.symbols
            .binary_search_by_key(&addr, |a| a.0)
            .map(|idx| self.symbols[idx].1.clone())
            .ok()
    }

    pub fn get_symbol_addr(&self, symbol: &str) -> Option<u64> {
        self.symbols.iter().find(|x| x.1 == symbol).map(|x| x.0)
    }

    fn disassemble_inst(&self, inst: Inst, pc: u64) -> String {
        let mut writer = String::new();

        let mut idx = self.symbols.partition_point(|a| a.0 < pc);
        if let Some(mut symbol) = self.symbols.get(idx) {
            while symbol.0 == pc {
                writer.push_str(&format!("{}:\n", symbol.1));

                idx += 1;
                symbol = &self.symbols[idx];
            }
        }

        writer.push_str(&format!("{pc:16x} {}", inst.fmt(pc)));

        let label_offset = match inst {
            Inst::Jalr {
                rd: _,
                rs1: _,
                offset,
            } => {
                let dest = pc.wrapping_add(offset as u64);
                Some(dest)
            }
            Inst::Jal { rd: _, offset } => {
                let dest = pc.wrapping_add(offset as u64);
                Some(dest)
            }
            _ => None,
        };

        if let Some(label_offset) = label_offset {
            if let Some(symbol) = self.get_symbol_at_addr(label_offset) {
                writer.push_str(&format!(" ; {symbol}"));
            }
        }

        writer
    }
}
