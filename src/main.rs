#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use elf::{endian::AnyEndian, ElfBytes};
use log::{debug, error, info};
use memory::Memory;

mod memory;

const SP: usize = 2;
const A0: usize = 10;
const A1: usize = 11;
const A2: usize = 12;
const A3: usize = 13;
const A4: usize = 14;
const A5: usize = 15;
const A6: usize = 16;
const A7: usize = 17;

const STACK_START: u64 = 0x8000000000000000;

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

        // STACK_START is actually inaccurate since it's actually the start of the kernel space memory.
        // So we subtract 8 to get the actual first valid memory address.
        em.reg[SP] = STACK_START - 256;

        em
    }

    // emulates linux syscalls
    fn syscall(&mut self, id: u64) {
        let arg = self.reg[A0];

        match id {
            // FACCESSAT
            48 => {
                self.reg[A0] = 0;
                // TODO: currently just noop (maybe that's fine, who knows)
            }

            // EXIT
            93 => {
                self.exit_code = Some(arg);
            }

            // EXIT_GROUP
            94 => {
                self.exit_code = Some(arg);
            }

            // SET_TID_ADDRESS
            96 => {
                // noop
            }

            // SET_ROBUST_LIST
            99 => {
                // noop
            }

            // BRK - man 2 brk
            214 => {
                self.reg[A0] = self.memory.brk(arg);
            }

            // NMAP - man 2 mmap
            // A0 - Page address, 0 if let os decide. Ignored.
            // A1 - Allocated buffer size
            222 => {
                self.reg[A0] = self.memory.mmap(self.reg[A1]);
            }

            _ => {
                unimplemented!("syscall {id} not implemented.");
            }
        }
    }

    fn fetch_and_execute(&mut self) -> Option<u64> {
        let inst = self.memory.load_u32(self.pc);
        self.print_registers();
        self.execute(inst);

        // let mut res = String::new();
        // std::io::stdin().read_line(&mut res).unwrap();

        self.fuel_counter += 1;
        self.exit_code
    }

    #[allow(unused)]
    fn print_registers(&self) {
        println!("stack: {:x?}", self.memory.stack);
        println!("heap_end: {:x?}", self.memory.heap_pointer);
        println!("dynamic data: {:x?}", self.memory.mmap_regions);
        println!("fuel consumed: {}", self.fuel_counter);
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

                // C.JR - ret
                if imm == 0 && rs1 != 0 && rs2 == 0 {
                    debug!("{:016x} jalr  x0, x{}, 0", self.pc, rs1);
                    // self.reg[0] = self.pc.wrapping_add(4);
                    self.pc = (self.reg[rs1 as usize] & (!0b1)) - 2;
                }
                // C.MV - Move
                else if imm == 0 && rs1 != 0 && rs2 != 0 {
                    debug!("{:016x} add   x{}, x0, x{}", self.pc, rs1, rs2);
                    self.reg[rs1] = self.reg[rs2];
                }
                // C.ADD - Add
                else if imm == 1 && rs1 != 0 && rs2 != 0 {
                    debug!("{:016x} add   x{}, x{}, x{}", self.pc, rs1, rs1, rs2);
                    self.reg[rs1] = self.reg[rs1] + self.reg[rs2];
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
                let imm = (inst & 0b1000000000000) >> 7 // imm[5]
                        | (inst & 0b11100) << 4 // imm[8:6]
                        | (inst & 0b1100000) >> 2; // imm[4:3]

                if rd != 0 {
                    // C.LDSP
                    debug!("{:016x} ld    x{}, {}(sp)", self.pc, rd, imm);
                    self.reg[rd as usize] = self
                        .memory
                        .load_u64((imm as u64).wrapping_add(self.reg[SP]));
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
                    .store_u64(offset.wrapping_add(self.reg[SP]), self.reg[rs1 as usize]);
            }

            (0b01, 0b000) => {
                // C.ADDI

                let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                        | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                let rd = (inst >> 7) & 0b11111;

                debug!("{:016x} addi  x{}, x{}, {}", self.pc, rd, rd, imm);
                self.reg[rd as usize] = self.reg[rd as usize].wrapping_add(imm as u64);
            }

            (0b01, 0b001) => {
                // C.ADDIW

                let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                        | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                let rd = (inst >> 7) & 0b11111;

                debug!("{:016x} addiw x{}, x{}, {}", self.pc, rd, rd, imm);

                self.reg[rd as usize] =
                    (self.reg[rd as usize] as u32).wrapping_add(imm as u32) as i32 as u64;
            }

            (0b01, 0b010) => {
                // C.LI - Load Immediate
                let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                        | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                let rd = (inst >> 7) & 0b11111;

                debug!("{:016x} li    x{}, {}", self.pc, rd, imm);
                self.reg[rd as usize] = imm as u64;
            }

            (0b01, 0b011) => {
                let rd = (inst >> 7) & 0b11111;

                if rd == 2 {
                    // C.ADDI16SP
                    let imm = (((inst & 0b1000000000000) << 3) as i16 >> 6) as u64 // imm[9]
                            | ((inst & 0b100) << 3) as u64 // imm[5]
                            | ((inst & 0b11000) << 4) as u64 // imm[8:7]
                            | ((inst & 0b100000) << 1) as u64 // imm[6]
                            | ((inst & 0b1000000) >> 2) as u64; // imm[4]

                    self.reg[SP] = self.reg[SP].wrapping_add(imm);

                    debug!("{:016x} add   sp, sp, {}", self.pc, imm as i64);
                } else {
                    // C.LUI
                    let imm = ((((inst & 0b1000000000000) << 3) as i16 as i32) << 2)  // imm[17]
                            | ((inst as u32 & 0b1111100) << 10) as i32; // imm[16:12]

                    debug!("{:016x} lui   x{}, 0x{:x}", self.pc, rd, imm);
                    self.reg[rd as usize] = imm as u64;
                }
            }

            // MATH BOY
            (0b01, 0b100) => {
                let funct2 = (inst >> 10) & 0b11;
                let rd = (((inst >> 7) & 0b111) + 8) as usize;

                match funct2 {
                    // C.SRLI
                    0b00 => {
                        let imm = (inst & 0b1000000000000) >> 7 // imm[5]
                                | (inst & 0b1111100) >> 2; // imm[4:0]

                        if imm == 0 {
                            panic!("Immediate must be nonzero");
                        }

                        debug!("{:016x} srli  x{}, x{}, {}", self.pc, rd, rd, imm);
                        self.reg[rd] = self.reg[rd] >> imm;
                    }

                    // C.ANDI
                    0b10 => {
                        let imm = ((inst & 0b1000000000000) << 3) as i16 >> 10 // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                        debug!("{:016x} andi  x{}, x{}, {}", self.pc, rd, rd, imm);
                        self.reg[rd] = self.reg[rd] & imm as u64;
                    }

                    0b11 => {
                        let funct2_2 = (inst >> 5) & 0b11;
                        let imm_bit = (inst >> 12) & 0b1;
                        let rs2 = (((inst >> 2) & 0b111) + 8) as usize;

                        match (imm_bit, funct2_2) {
                            // C.SUB
                            (0, 0b00) => {
                                self.reg[rd] = self.reg[rd].wrapping_sub(self.reg[rs2]);
                                debug!("{:016x} sub   x{}, x{}, x{}", self.pc, rd, rd, rs2);
                            }

                            // C.XOR
                            (0, 0b01) => {
                                self.reg[rd] = self.reg[rd] ^ self.reg[rs2];
                                debug!("{:016x} xor   x{}, x{}, x{}", self.pc, rd, rd, rs2);
                            }

                            // C.OR
                            (0, 0b10) => {
                                self.reg[rd] = self.reg[rd] | self.reg[rs2];
                                debug!("{:016x} or    x{}, x{}, x{}", self.pc, rd, rd, rs2);
                            }

                            // C.AND
                            (0, 0b11) => {
                                self.reg[rd] = self.reg[rd] & self.reg[rs2];
                                debug!("{:016x} and   x{}, x{}, x{}", self.pc, rd, rd, rs2);
                            }

                            // C.SUBW
                            (1, 0b00) => {
                                self.reg[rd] =
                                    (self.reg[rd] as i32).wrapping_sub(self.reg[rs2] as i32) as u64;
                                debug!("{:016x} subw  x{}, x{}, x{}", self.pc, rd, rd, rs2);
                            }

                            // C.ADDW
                            (1, 0b01) => {
                                self.reg[rd] =
                                    (self.reg[rd] as i32).wrapping_sub(self.reg[rs2] as i32) as u64;
                                debug!("{:016x} addw  x{}, x{}, x{}", self.pc, rd, rd, rs2);
                            }

                            _ => {
                                unreachable!();
                            }
                        }
                    }
                    _ => {
                        unimplemented!("{inst:0b} {funct2:0b}");
                    }
                }
            }

            (0b01, 0b101) => {
                let imm = (inst & 0b100) << 3 // imm[5]
                        | (inst & 0b111000) >> 2 // imm[3:1]
                        | (inst & 0b1000000) << 1 // imm[7]
                        | (inst & 0b10000000) >> 1 // imm[6]
                        | (inst & 0b100000000) << 2 // imm[10]
                        | (inst & 0b11000000000) >> 1 // imm[9:8]
                        | (inst & 0b100000000000) >> 7 // imm[4]
                        | (((inst & 0b1000000000000) << 3) as i16 >> 4) as u16; // imm[11]

                let addr = self.pc.wrapping_add(imm as i16 as u64);

                debug!("{:016x} j     {:x}", self.pc, addr);

                self.pc = addr.wrapping_sub(2);
            }

            (0b01, 0b110) => {
                // !!! BRANCH IMPLEMENTATION !!!
                // C.BEQZ

                let imm = ((inst & 0b110000000000) >> 7) as u64 // imm[4:3]
                        | (((inst & 0b1000000000000) >> 5) as i8 as u64) << 1 // imm[8]
                        | ((inst & 0b100) << 3) as u64 // imm[5]
                        | ((inst & 0b11000) >> 2) as u64 // imm[2:1]
                        | ((inst & 0b1100000) << 1) as u64; // imm[7:6]

                let rs1 = ((inst >> 7) & 0b111) + 8;

                let addr = self.pc.wrapping_add(imm as u64);

                debug!("{:016x} bnez  x{}, {:x}", self.pc, rs1, addr);

                if self.reg[rs1 as usize] == 0 {
                    self.pc = addr.wrapping_sub(2);
                }
            }

            (0b01, 0b111) => {
                // !!! BRANCH IMPLEMENTATION !!!
                // C.BNEZ

                let imm = ((inst & 0b110000000000) >> 7) as u64 // imm[4:3]
                        | (((inst & 0b1000000000000) >> 5) as i8 as u64) << 1 // imm[8]
                        | ((inst & 0b100) << 3) as u64 // imm[5]
                        | ((inst & 0b11000) >> 2) as u64 // imm[2:1]
                        | ((inst & 0b1100000) << 1) as u64; // imm[7:6]

                let rs1 = ((inst >> 7) & 0b111) + 8;

                let addr = self.pc.wrapping_add(imm as u64);

                debug!("{:016x} bnez   x{}, {:x}", self.pc, rs1, addr);

                if self.reg[rs1 as usize] != 0 {
                    self.pc = addr.wrapping_sub(2);
                }
            }

            // (0b10, 0b000) => {
            //     // C.SLLI
            // }
            (0b00, 0b000) => {
                // C.ADDI4SPN

                // nzuimm
                let imm = (inst & 0b100000) >> 2 // imm[3]
                        | (inst & 0b1000000) >> 4 // imm[2]
                        | (inst & 0b11110000000) >> 1 // imm[9:6]
                        | (inst & 0b1100000000000) >> 7; // imm[5:4]

                let rd = ((inst >> 2) & 0b111) + 8;

                debug!("{:016x} addi  x{}, sp, {}", self.pc, rd, imm);
                self.reg[rd as usize] = self.reg[SP] + imm as u64;
            }

            (0b00, 0b010) => {
                // C.LW
                let rd = ((inst >> 2) & 0b111) + 8;
                let rs1 = ((inst >> 7) & 0b111) + 8;

                // uimm
                let imm = (inst & 0b100000) << 1 // imm[6]
                        | (inst & 0b1000000) >> 4 // imm[2]
                        | (inst & 0b1110000000000) >> 7; // imm[5:3]

                debug!("{:016x} lw    x{}, {}(x{}) (c.ld)", self.pc, rd, imm, rs1);

                let addr = self.reg[rs1 as usize].wrapping_add(imm as u64);
                self.reg[rd as usize] = self.memory.load_u32(addr) as i32 as u64;
            }

            (0b00, 0b011) => {
                // C.LD
                let rd = ((inst >> 2) & 0b111) + 8;
                let rs1 = ((inst >> 7) & 0b111) + 8;

                // uimm
                let imm = ((inst >> 7) & 0b111000) | (((inst >> 5) & 0b111) << 6);

                debug!("{:016x} ld    x{}, {}(x{}) (c.ld)", self.pc, rd, imm, rs1);

                let addr = self.reg[rs1 as usize].wrapping_add(imm as u64);
                self.reg[rd as usize] = self.memory.load_u64(addr) as u64;
            }

            (0b00, 0b110) => {
                // C.SW

                // uimm
                let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                        | (inst & 0b100000) << 1 // imm[6]
                        | (inst & 0b1000000) >> 4; // imm[2]

                let rs1 = ((inst >> 7) & 0b111) + 8;
                let rs2 = ((inst >> 2) & 0b111) + 8;

                let addr = self.reg[rs1 as usize].wrapping_add(imm as u64);

                debug!("{:016x} sd    x{}, {}(x{}) (c.sw)", self.pc, rs2, imm, rs1);

                self.memory.store_u32(addr, self.reg[rs2 as usize] as u32);
            }

            (0b00, 0b111) => {
                // C.SD

                // uimm
                let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                        | (inst & 0b1100000) << 1; // imm[7:6]

                let rs1 = ((inst >> 7) & 0b111) + 8;
                let rs2 = ((inst >> 2) & 0b111) + 8;

                let addr = self.reg[rs1 as usize].wrapping_add(imm as u64);

                debug!("{:016x} sd    x{}, {}(x{}) (c.sd)", self.pc, rs2, imm, rs1);

                self.memory.store_u64(addr, self.reg[rs2 as usize]);
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
                    // LW
                    0b010 => {
                        debug!("{:016x} lw    x{}, {}(x{})", self.pc, rd, offset, rs1);
                        self.reg[rd] = self.memory.load_u32(addr) as i32 as u64;
                    }
                    // LD
                    0b011 => {
                        debug!("{:016x} ld    x{}, {}(x{})", self.pc, rd, offset, rs1);
                        self.reg[rd] = self.memory.load_u64(addr) as u64;
                    }

                    // LBU
                    0b100 => {
                        debug!("{:016x} lbu   x{}, {}(x{})", self.pc, rd, offset, rs1);
                        self.reg[rd] = self.memory.load_byte(addr) as i8 as u64;
                    }

                    0b101 => {
                        debug!("{:016x} lhu   x{}, {}(x{})", self.pc, rd, offset, rs1);
                        self.reg[rd] = self.memory.load_u16(addr) as u64;
                    }

                    _ => {
                        unimplemented!("{funct3:b}")
                    }
                }
            }

            // FENCE
            0b0001111 => {
                // noop
            }

            0b0010011 => {
                let imm = (inst & 0xFFF00000) as i32 as i64 >> 20;
                match funct3 {
                    // ADDI
                    0b000 => {
                        debug!("{:016x} addi  x{}, x{}, {}", self.pc, rd, rs1, imm);
                        self.reg[rd] = self.reg[rs1].wrapping_add(imm as u64);
                    }
                    // SLLI
                    0b001 => {
                        let shamt = (inst >> 20) & 0b11111;

                        debug!("{:016x} slli  x{}, x{}, {}", self.pc, rd, rs1, imm);
                        self.reg[rd] = self.reg[rs1] << shamt;
                    }
                    0b111 => {
                        debug!("{:016x} andi  x{}, x{}, {}", self.pc, rd, rs1, imm);
                        self.reg[rd] = self.reg[rs1] & imm as u64;
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

            0b0011011 => {
                // let rs1 = (inst >> 15) & 0b11111;
                // let rd = (inst >> 7) & 0b11111;

                match funct3 {
                    // ADDIW
                    0b000 => {
                        let imm = ((inst & 0b11111111111100000000000000000000) as i32 >> 20) as u64;

                        debug!("{:016x} addiw x{}, x{}, {}", self.pc, rd, rs1, imm as i32);
                        self.reg[rd] = self.reg[rs1].wrapping_add(imm);
                    }

                    // SLLIW
                    0b001 => {
                        let shamt = (inst >> 20) & 0b111111;

                        debug!("{:016x} slliw x{}, x{}, {}", self.pc, rd, rs1, shamt);
                        self.reg[rd] = ((self.reg[rs1] as i32) << shamt) as u64;
                    }

                    _ => {
                        unimplemented!("0011011 funct3={funct3}");
                    }
                }
            }

            // STORE
            0b0100011 => {
                let imm = ((inst & 0b11111110000000000000000000000000) as i32) >> 20 // imm[11:5]
                        | (inst & 0b111110000000) as i32 >> 7; // imm[4:0]
                let addr = self.reg[rs1].wrapping_add(imm as u64);

                match funct3 {
                    // SD
                    0b011 => {
                        debug!("{:016x} sd    x{}, {}(x{})", self.pc, rs2, imm, rs1);
                        self.memory.store_u64(addr, self.reg[rs2]);
                    }
                    // SB
                    0b000 => {
                        debug!("{:016x} sb    x{}, {}(x{})", self.pc, rs2, imm, rs1);
                        self.memory.store_byte(addr, self.reg[rs2] as u8);
                    }
                    // SH
                    0b001 => {
                        unimplemented!("SH unimplemented");
                    }
                    // SW
                    0b010 => {
                        debug!("{:016x} sw    x{}, {}(x{})", self.pc, rs2, imm, rs1);
                        self.memory.store_u32(addr, self.reg[rs2] as u32);
                    }
                    _ => {
                        unimplemented!("Unknown store instruction unimplemented");
                    }
                }
            }

            // ADD
            0b0110011 => {
                debug!("{:016x} add   x{}, x{}, x{}", self.pc, rd, rs1, rs2);
                self.reg[rd] = self.reg[rs1].wrapping_add(self.reg[rs2]);
            }

            // !!! BRANCH IMPLEMENTATION !!!
            0b1100011 => {
                let imm = (inst & 0b1111110000000000000000000000000) >> 20  // imm[10:5]
                        | ((inst & 0b10000000000000000000000000000000) as i32 >> 19) as u32 // imm[12]
                        | (inst & 0b10000000) << 4 // imm[11]
                        | (inst & 0b111100000000) >> 7; // imm[4:1]

                let rs2 = ((inst >> 20) & 0b11111) as usize;
                let rs1 = ((inst >> 15) & 0b11111) as usize;

                let addr = self.pc.wrapping_add(imm as i32 as u64);

                let condition;

                match funct3 {
                    // BEQ
                    0b000 => {
                        debug!("{:016x} beq   x{}, x{}, {:x}", self.pc, rs1, rs2, addr);
                        condition = self.reg[rs1] == self.reg[rs2];
                    }
                    // BNE
                    0b001 => {
                        debug!("{:016x} bne   x{}, x{}, {:x}", self.pc, rs1, rs2, addr);
                        condition = self.reg[rs1] != self.reg[rs2];
                    }
                    // BLT
                    0b100 => {
                        debug!("{:016x} blt   x{}, x{}, {:x}", self.pc, rs1, rs2, addr);
                        condition = (self.reg[rs1] as i64) < self.reg[rs2] as i64;
                    }
                    // BGE
                    0b101 => {
                        debug!("{:016x} bge   x{}, x{}, {:x}", self.pc, rs1, rs2, addr);
                        condition = (self.reg[rs1] as i64) >= self.reg[rs2] as i64;
                    }
                    // BLTU
                    0b110 => {
                        debug!("{:016x} bltu  x{}, x{}, {:x}", self.pc, rs1, rs2, addr);
                        condition = self.reg[rs1] < self.reg[rs2];
                    }
                    // BGEU
                    0b111 => {
                        debug!("{:016x} bgeu  x{}, x{}, {:x}", self.pc, rs1, rs2, addr);
                        condition = self.reg[rs1] >= self.reg[rs2];
                    }

                    _ => {
                        unimplemented!("Unknown instruction encoding, func3 for BRANCH")
                    }
                }

                if condition {
                    self.pc = addr.wrapping_sub(4);
                }
            }

            // JAL - Jump Address Long
            0b1101111 => {
                // 20 byte immediate, signed, shifted once

                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = (((inst & 0x80000000) as i32 as i64 >> 11) as u64) // imm[20]
                    | (inst & 0xff000) as u64 // imm[19:12]
                    | ((inst >> 9) & 0x800) as u64 // imm[11]
                    | ((inst >> 20) & 0x7fe) as u64; // imm[10:1]

                let addr = self.pc.wrapping_add(imm as u64);

                debug!("{:016x} jal   x{}, {:x}", self.pc, rd, addr);
                self.reg[rd] = self.pc.wrapping_add(4);
                self.pc = addr.wrapping_sub(4); // we subtract 4 here to account for what's added to the pc at the end of this function
            }

            // ECALL - Execute syscall
            0b1110011 => {
                let id = self.reg[A7];

                self.syscall(id);
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

    let file_entry = file.ehdr.e_entry;
    let memory = Memory::load_elf(file);
    let mut emulator = Emulator::new(file_entry, memory);

    loop {
        if let Some(exit_code) = emulator.fetch_and_execute() {
            println!("Program exited with code {exit_code}");
            break;
        }
    }

    emulator.print_registers();

    Ok(())
}
