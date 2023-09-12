#![forbid(unsafe_code)]

use std::rc::Rc;

use anyhow::Result;
use clap::Parser;
use elf::{endian::AnyEndian, ElfBytes};
use log::{debug, error, info};

const SP: usize = 2;
const STACK_START: u64 = 0x7fffffffffffffff;

// addressing
struct Emulator {
    pc: u64,
    reg: [u64; 32],
    memory: Memory,

    exit_code: Option<u64>,

    /// The number of instructions executed over the lifecycle of the emulator.
    fuel_counter: u64,
    /// Similar to fuel_counter, but also takes into account intruction level parallelism and cache misses.
    performance_counter: u64,
}

struct Memory {
    text_addr: u64,
    text: Box<[u8]>,
    data: Box<[u8]>,

    // reverse indexed
    stack: Vec<u8>,
    // heap: Vec<u8>,
}

impl Memory {
    fn new(text_addr: u64, text: &[u8]) {}

    fn load_u64(&self, index: u64) -> u64 {
        return (self.load_u32(index) as u64) | ((self.load_u32(index + 4) as u64) << 32);
    }

    fn load_u32(&self, index: u64) -> u32 {
        // let index = self.pc;
        return (self.load_byte(index) as u32)
            | ((self.load_byte(index + 1) as u32) << 8)
            | ((self.load_byte(index + 2) as u32) << 16)
            | ((self.load_byte(index + 3) as u32) << 24);
    }

    fn load_byte(&self, idx: u64) -> u8 {
        if idx >= self.text_addr && idx <= self.text_addr + self.text.len() as u64 {
            self.text[(idx - self.text_addr) as usize]
        } else if idx <= STACK_START {
            self.stack[(STACK_START - idx) as usize]
        } else {
            0
        }
    }

    fn store_u64(&mut self, index: u64, data: u64) {
        self.store_u32(index, (data >> 32) as u32);
        self.store_u32(index + 4, (data >> 32) as u32);
    }

    fn store_u32(&mut self, index: u64, data: u32) {
        self.store_byte(index, (data >> 24) as u8);
        self.store_byte(index + 1, (data >> 16) as u8);
        self.store_byte(index + 2, (data >> 8) as u8);
        self.store_byte(index + 3, (data) as u8);
    }

    fn store_byte(&mut self, idx: u64, data: u8) {
        if idx >= self.text_addr && idx <= self.text_addr + self.text.len() as u64 {
            panic!("Attempting to store memory in read-only location: {}", idx);
        } else if idx <= STACK_START {
            let real_idx = (STACK_START - idx) as usize;

            if self.stack.len() < real_idx {
                self.stack.resize(real_idx, 0);
            }

            self.stack[real_idx as usize] = data;
        }
    }
}

impl Emulator {
    fn new(entry: u64, memory: Memory) -> Self {
        let mut em = Self {
            pc: entry,
            reg: [0; 32],
            memory,
            exit_code: None,
            fuel_counter: 0,
            performance_counter: 0,
        };

        em.reg[SP] = STACK_START;

        em
    }

    // emulates linux syscalls
    fn syscall(&mut self, id: u64, argument: u64) {
        match id {
            // EXIT
            93 => {
                self.exit_code = Some(argument);
            }
            _ => {
                unimplemented!("syscall {id} not implemented.");
            }
        }
    }

    fn fetch_and_execute(&mut self) -> Option<u64> {
        let inst = self.memory.load_u32(self.pc);
        self.print_registers();
        // println!("{:08x?}", inst);
        self.execute(inst);

        self.exit_code
    }

    #[allow(unused)]
    fn print_registers(&self) {
        println!("x0 (zero):  {:x}", self.reg[0]);
        println!("x1 (ra):    {:x}", self.reg[1]);
        println!("x2 (sp):    {:x}", self.reg[2]);
        println!("x3 (gp):    {:x}", self.reg[3]);
        println!("x4 (tp):    {:x}", self.reg[4]);
        println!("x5 (t0):    {:x}", self.reg[5]);
        println!("x6 (t1):    {:x}", self.reg[6]);
        println!("x7 (t2):    {:x}", self.reg[7]);
        println!("x8 (s0/fp): {:x}", self.reg[8]);
        println!("x9 (s1):    {:x}", self.reg[9]);
        println!("x10 (a0):   {:x}", self.reg[10]);
        println!("x11 (a1):   {:x}", self.reg[11]);
        println!("x12 (a2):   {:x}", self.reg[12]);
        println!("x13 (a3):   {:x}", self.reg[13]);
        println!("x14 (a4):   {:x}", self.reg[14]);
        println!("x15 (a5):   {:x}", self.reg[15]);
        println!("x16 (a6):   {:x}", self.reg[16]);
        println!("x17 (a7):   {:x}", self.reg[17]);
        println!("x18 (s2):   {:x}", self.reg[18]);
        println!("x19 (s3):   {:x}", self.reg[19]);
        println!("x20 (s4):   {:x}", self.reg[20]);
        println!("x21 (s5):   {:x}", self.reg[21]);
        println!("x22 (s6):   {:x}", self.reg[22]);
        println!("x23 (s7):   {:x}", self.reg[23]);
        println!("x24 (s8):   {:x}", self.reg[24]);
        println!("x25 (s9):   {:x}", self.reg[25]);
        println!("x26 (s10):  {:x}", self.reg[26]);
        println!("x27 (s11):  {:x}", self.reg[27]);
        println!("x28 (t3):   {:x}", self.reg[28]);
        println!("x29 (t4):   {:x}", self.reg[29]);
        println!("x30 (t5):   {:x}", self.reg[30]);
        println!("x31 (t6):   {:x}", self.reg[31]);
    }

    /// The least significant two bits demarkate whether an instruction is compressed or not.
    fn execute(&mut self, inst: u32) {
        match inst & 0b11 {
            0b00 | 0b01 | 0b10 => {
                self.execute_compressed(inst as u16);
                self.pc += 2;
            }

            _ => {
                self.execute_general(inst);
                self.pc += 4;
            }
        }

        // make sure x0 is zero
        self.reg[0] = 0;
    }

    fn execute_compressed(&mut self, inst: u16) {
        let quadrant = inst & 0b11;
        let funct3 = (inst >> 13) & 0b111;

        match (quadrant, funct3) {
            (0b10, 0b000) => {
                // C.SLLI
                let shift_amount = (((inst >> 12) & 0b1) << 5) | ((inst >> 2) & 0b1111);
                let rd = ((inst >> 7) & 0b11111) as usize;

                self.reg[rd] = self.reg[rd] << shift_amount as u64;

                debug!("{:016x} slli  x{}, x{}, {}", self.pc, rd, rd, shift_amount);
            }
            (0b10, 0b100) => {
                let imm = (inst >> 12) & 0b1;
                let rs1 = ((inst >> 7) & 0b11111) as usize;
                let rs2 = ((inst >> 2) & 0b11111) as usize;

                // C.JR - Jump Regsiter
                if imm == 0 && rs1 != 0 && rs2 == 0 {
                    debug!("{:016x} jalr  x0, x{}, 0", self.pc, rs1);
                    // self.reg[0] = self.pc.wrapping_add(4);
                    self.pc = self.reg[rs1 as usize] & (!0b1);
                }
                // C.MV - Move
                else if imm == 0 && rs1 != 0 && rs2 != 0 {
                    debug!("{:016x} add   x{}, x0, x{}", self.pc, rs1, rs2);
                    self.reg[rs1] = self.reg[rs2];
                } else {
                    log::info!("funct3={funct3:03b}, quadrant={quadrant:02b}");
                    error!(
                        "{:016x} compressed instruction `{inst:016b}` not implemented.",
                        self.pc
                    );
                    unimplemented!();
                }
            }

            (0b10, 0b011) => {
                let rd = (inst >> 7) & 0b11111;
                let offset = ((inst >> 12 & 0b1) << 4) // offset 5
                | ((inst >> 5 & 0b11) << 2) // offset[4:3]
                | ((inst >> 2 & 0b111) << 5); // offset[8:6]

                if rd != 0 {
                    // C.LDSP
                    debug!("{:016x} ld    x{}, {}(sp)", self.pc, rd, offset << 1);
                    self.reg[rd as usize] = self
                        .memory
                        .load_u64((offset << 1) as u64)
                        .wrapping_add(self.reg[SP]);
                } else {
                    error!("C.FLWSP not implemented");
                }
                //
            }

            (0b10, 0b111) => {
                // C.SDSP - not C.SWSP since we are emulating RV64C
                let offset = (((inst >> 7) & 0b111000) | ((inst >> 1) & 0b111000000)) as u64;
                let rs1 = (inst >> 2) & 0b11111;

                debug!(
                    "{:016x} sd    x{}, {}(sp) sp=0x{:x}",
                    self.pc, rs1, offset, self.reg[SP]
                );

                self.memory
                    .store_u64(offset + self.reg[SP], self.reg[rs1 as usize]);
            }

            (0b01, 0b010) => {
                // C.LI - Load Immediate
                let imm = ((inst >> 12) & 0b1) << 6 | ((inst >> 2) & 0b11111);
                let rd = (inst >> 7) & 0b11111;

                debug!("{:016x} li    x{}, {}", self.pc, rd, imm);
                self.reg[rd as usize] = imm as u64;
            }

            (0b01, 0b011) => {
                let rd = (inst >> 7) & 0b11111;

                if rd == 2 {
                    // C.ADDI16SP
                    let imm = (((inst >> 12) & 0b1) << 8)
                        | (((inst >> 3) & 0b11) << 7)
                        | (((inst >> 5) & 0b1) << 6)
                        | (((inst >> 2) & 0b1) << 5)
                        | (((inst >> 6) & 0b1) << 4);

                    let imm = imm as i64 + -512; // adapt to range (-512, 496)
                    self.reg[SP] = self.reg[SP].wrapping_add(imm as u64);

                    debug!("{:016x} add   sp, sp, {}", self.pc, imm);
                } else {
                    let imm = ((((inst >> 11) & 0b1) << 4) | ((inst >> 2) & 0b11111)) as u64;
                    // C.LUI - Sign extended (don't currently know exactly how to do that)
                    debug!("{:016x} lui   x{}, 0x{:x}", self.pc, rd, imm << 12);
                    self.reg[rd as usize] = imm << 12;
                }
            }

            // (0b10, 0b000) => {
            //     // C.SLLI
            // }
            //
            (0b00, 0b000) => {
                // C.ADDI4SPN
                let imm = ((inst >> 4) & 0xFF) << 2;
                let rd = (inst >> 2) & 0b111;

                debug!("{:016x} addi  x{}, sp, {}", self.pc, rd, imm);
                self.reg[rd as usize] = self.reg[SP] + imm as u64;
            }
            _ => {
                log::info!("funct3={funct3:03b}, quadrant={quadrant:02b}");
                error!(
                    "{:016x} compressed instruction `{inst:016b}` not implemented.",
                    self.pc
                );
                unimplemented!();
            }
        }
    }

    fn execute_general(&mut self, inst: u32) {
        let opcode = inst & 0b1111111;
        let rd = ((inst >> 7) & 0b11111) as usize;
        let rs1 = ((inst >> 15) & 0b11111) as usize;
        let rs2 = ((inst >> 20) & 0b11111) as usize;

        let funct3 = (inst >> 12) & 0b111;
        // println!("{inst:032b} {inst:x}");

        match opcode {
            // LOAD, LD, LW, etc
            0b0000011 => {
                // imm[11:0]
                let offset = ((inst & 0xFFF00000) as i32 as i64) >> 20;
                let addr = self.reg[rs1].wrapping_add(offset as u64);

                debug!("{} + {} = {}", self.reg[rs1], offset, addr);

                match funct3 {
                    // byte
                    // 0b000 => {}
                    // half word (16 bits)
                    // 0b001 => {}
                    // word (32 bits)
                    0b010 => {
                        debug!("{:016x} lw    x{}, {}(x{})", self.pc, rd, offset, rs1);
                        self.reg[rd] = self.memory.load_u32(addr) as u64;
                    }
                    // double word (64 bits)
                    0b011 => {
                        debug!("{:016x} ld    x{}, {}(x{})", self.pc, rd, offset, rs1);
                        self.reg[rd] = self.memory.load_u64(addr) as u64;
                    }

                    _ => {
                        unimplemented!()
                    }
                }
            }

            // ADD
            0b0110011 => {
                debug!("{:016x} add   x{}, x{}, x{}", self.pc, rd, rs1, rs2);
                self.reg[rd] = self.reg[rs1].wrapping_add(self.reg[rs2]);
            }

            // ADDI
            0b0010011 => {
                match funct3 {
                    0b000 => {
                        // 12 byte immediate, signed
                        let imm = (inst & 0xFFF00000) as i32 as i64 >> 20;
                        debug!("{:016x} addi  x{}, x{}, {}", self.pc, rd, rs1, imm);
                        self.reg[rd] = self.reg[rs1].wrapping_add(imm as u64);
                    }
                    0b111 => {
                        debug!("{:016x} andi  x{}, x{}, x{}", self.pc, rd, rs1, rs2);
                        self.reg[rd] = self.reg[rs1] & self.reg[rs2];
                    }
                    _ => {
                        unimplemented!(
                            "func3 for ADDI instruction not implemented: {:03b}",
                            funct3
                        );
                    }
                }
            }

            // AUIPC - Add Upper Immediate to PC
            0b0010111 => {
                // imm[31:12] = inst[31:12]
                let imm = (inst & 0xFFFFF000) as i32 as i64 as u64;
                debug!("{:016x} auipc x{}, 0x{:x}", self.pc, rd, imm);
                self.reg[rd] = self.pc.wrapping_add(imm);
            }

            // JAL - Jump Address Long
            0b1101111 => {
                // 20 byte immediate, signed, shifted once

                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = (((inst & 0x80000000) as i32 as i64 >> 11) as u64) // imm[20]
                    | (inst & 0xff000) as u64 // imm[19:12]
                    | ((inst >> 9) & 0x800) as u64 // imm[11]
                    | ((inst >> 20) & 0x7fe) as u64; // imm[10:1]

                debug!("{:016x} jal   x{}, 0x{:x}", self.pc, rd, imm);
                self.reg[rd] = self.pc.wrapping_add(4);
                self.pc = self.pc.wrapping_add(imm as u64).wrapping_sub(4);
            }

            // ECALL - Execute syscall
            0b1110011 => {
                let id = self.reg[17];
                let arg = self.reg[10];

                self.syscall(id, arg);
            }

            _ => {
                error!("{:016x} opcode `{opcode:07b}` not implemented.", self.pc);
                panic!();
            }
        }
    }
}

#[derive(Parser)]
struct Arguments {
    file: String,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

fn main() -> Result<()> {
    let args = Arguments::parse();

    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();

    let file_data = std::fs::read(args.file).expect("Could not read file.");
    let slice = file_data.as_slice();
    let file = ElfBytes::<AnyEndian>::minimal_parse(slice)?;

    match (file.ehdr.class, file.ehdr.e_type, file.ehdr.e_machine) {
        // (64 bit, executable, risc_v arch)
        (elf::file::Class::ELF64, 0x02, 0xF3) => info!("Parsing executable."),
        got => {
            eprintln!(
                "Error. Invalid executable format. Expects a 64-bit RISC-V Linux binary. Got: {:x?}",
                got
            );
            return Ok(());
        }
    }

    let text_header = file.section_header_by_name(".text")?.unwrap();
    let (text_data, text_compression_header) = file.section_data(&text_header)?;

    let data_header = file.section_header_by_name(".data")?.unwrap();
    let (data_data, data_compression_header) = file.section_data(&text_header)?;

    let bss_header = file.section_header_by_name(".bss")?.unwrap();
    let (bss_data, bss_compression_header) = file.section_data(&text_header)?;

    if text_compression_header.is_some()
        || data_compression_header.is_some()
        || bss_compression_header.is_some()
    {
        panic!("This emulator does not implement compression");
    }

    let file_entry = file.ehdr.e_entry;

    let mut emulator = Emulator::new(
        file_entry,
        Memory {
            text_addr: text_header.sh_addr,
            text: text_data.into(),
            stack: Vec::new(),
            data: [data_data, bss_data].concat().into(),
        },
    );

    // println!("text addr: {file:?}");

    loop {
        if let Some(exit_code) = emulator.fetch_and_execute() {
            println!("Program exited with code {exit_code}");
            break;
        }
    }

    emulator.print_registers();

    Ok(())
}
