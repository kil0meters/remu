use elf::{endian::EndianParse, ElfBytes};

use crate::{instruction::Inst, memory::MemMap};

const STT_FUNC: u8 = 2;

#[derive(Clone)]
pub struct Disassembler {
    symbols: MemMap<u64, String>,
    pub instructions: MemMap<u64, (Inst, u8)>,
    pc_line_mappings: MemMap<u64, u64>,
    text_regions: Vec<(u64, u64)>,

    disassembly_string: Option<Vec<Box<str>>>,
}

impl Disassembler {
    pub fn new() -> Disassembler {
        Disassembler {
            symbols: MemMap::default(),
            instructions: MemMap::default(),
            pc_line_mappings: MemMap::default(),
            text_regions: Vec::default(),

            disassembly_string: None,
        }
    }

    // offset: the address offset in memory
    pub fn add_elf<T: EndianParse>(&mut self, elf: &ElfBytes<T>, offset: u64) {
        // add symbols

        let (symbol_table, string_table) = elf.symbol_table().unwrap().unwrap();

        for symbol in symbol_table.iter() {
            if symbol.st_symtype() == STT_FUNC {
                let symbol_name = string_table.get(symbol.st_name as usize).unwrap();
                self.symbols
                    .insert(symbol.st_value + offset, symbol_name.to_string());
            }
        }

        for section_name in [".text", ".plt"] {
            // add instructions
            let section_header = elf
                .section_header_by_name(section_name)
                .unwrap()
                .expect("ELF file does not have a required section");

            let start = offset + section_header.sh_addr;
            let end = start + section_header.sh_size;
            self.text_regions.push((start, end));

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

                self.instructions.insert(pc as u64 + start, (inst, step));
                pc += step as usize;
            }
        }

        self.text_regions.sort_unstable_by_key(|a| a.0);
        self.disassembly_string = None;
    }

    pub fn get_inst_line(&self, pc: u64) -> u64 {
        *self.pc_line_mappings.get(&pc).unwrap_or(&0)
    }

    pub fn disassemble(&mut self) -> Vec<Box<str>> {
        if let Some(ref s) = self.disassembly_string {
            return s.clone();
        }

        let mut writer: Vec<Box<str>> = Vec::new();

        for (start, end) in &self.text_regions {
            let mut pc = *start;
            while pc < *end {
                let (inst, step) = self.instructions.get(&pc).unwrap();

                if let Some(label) = self.symbols.get(&pc) {
                    writer.push("".into());
                    writer.push(format!("{label}:").into());
                }

                let mut inst_string = String::new();
                inst_string.push_str(&format!("{pc:16x} {}", inst.fmt(pc)));

                let label_offset = match inst {
                    Inst::Jalr {
                        rd: _,
                        rs1: _,
                        offset,
                    } => {
                        let dest = pc.wrapping_add(*offset as u64);
                        Some(dest)
                    }
                    Inst::Jal { rd: _, offset } => {
                        let dest = pc.wrapping_add(*offset as u64);
                        Some(dest)
                    }
                    _ => None,
                };

                if let Some(label_offset) = label_offset {
                    if let Some(label) = self.symbols.get(&label_offset) {
                        inst_string.push_str(&format!(" ; {label}"));
                    }
                }

                self.pc_line_mappings.insert(pc, writer.len() as u64);
                writer.push(inst_string.into());
                pc += *step as u64;
            }

            writer.push("".into());
        }

        self.disassembly_string = Some(writer.clone());
        writer
    }
}
