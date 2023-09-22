use std::{
    fmt::Display,
    ops::{Index, IndexMut},
};

use crate::{
    auxvec::{AuxPair, Auxv, RANDOM_BYTES},
    instruction::Inst,
    memory::Memory,
    syscalls::{
        self, BRK, CLOCK_GETTIME, EXIT, EXIT_GROUP, FACCESSAT, GETRANDOM, MMAP, PRLIMIT64,
        READLINKAT, SET_ROBUST_LIST, SET_TID_ADDRESS, WRITE,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reg(pub u8);

impl<T> Index<Reg> for [T] {
    type Output = T;
    fn index(&self, index: Reg) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<T> IndexMut<Reg> for [T] {
    fn index_mut(&mut self, index: Reg) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

impl Display for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            0 => "x0",
            1 => "ra",
            2 => "sp",
            3 => "gp",
            4 => "tp",
            5 => "t0",
            6 => "t1",
            7 => "t2",
            8 => "s0",
            9 => "s1",
            10 => "a0",
            11 => "a1",
            12 => "a2",
            13 => "a3",
            14 => "a4",
            15 => "a5",
            16 => "a6",
            17 => "a7",
            18 => "s2",
            19 => "s3",
            20 => "s4",
            21 => "s5",
            22 => "s6",
            23 => "s7",
            24 => "s8",
            25 => "s9",
            26 => "s10",
            27 => "s11",
            28 => "t3",
            29 => "t4",
            30 => "t5",
            31 => "t6",
            _ => unreachable!(),
        };

        write!(f, "{s}")
    }
}

pub const SP: Reg = Reg(2);
pub const S0: Reg = Reg(8);
pub const S1: Reg = Reg(9);
pub const A0: Reg = Reg(10);
pub const A1: Reg = Reg(11);
pub const A2: Reg = Reg(12);
pub const A3: Reg = Reg(13);
pub const A4: Reg = Reg(14);
pub const A5: Reg = Reg(15);
pub const A6: Reg = Reg(16);
pub const A7: Reg = Reg(17);
pub const S2: Reg = Reg(18);
pub const S3: Reg = Reg(19);
pub const S4: Reg = Reg(20);
pub const S5: Reg = Reg(21);
pub const S6: Reg = Reg(22);
pub const S7: Reg = Reg(23);
pub const S8: Reg = Reg(24);
pub const S9: Reg = Reg(25);
pub const S10: Reg = Reg(26);
pub const S11: Reg = Reg(27);

pub const STACK_START: u64 = 0x7fffffffffffffff;
pub const USER_STACK_OFFSET: u64 = 201;

pub struct Emulator {
    pc: u64,
    x: [u64; 32],
    memory: Memory,

    exit_code: Option<u64>,

    /// The number of instructions executed over the lifecycle of the emulator.
    fuel_counter: u64,
    /// Similar to fuel_counter, but also takes into account intruction level parallelism and cache misses.
    performance_counter: u64,
}

impl Emulator {
    pub fn new(entry: u64, memory: Memory) -> Self {
        let mut em = Self {
            pc: entry,
            x: [0; 32],
            memory,
            exit_code: None,
            fuel_counter: 0,
            performance_counter: 0,
        };

        em.x[SP] = STACK_START;

        em.init_auxv_stack();

        em
    }

    // https://github.com/torvalds/linux/blob/master/fs/binfmt_elf.c#L175
    // https://github.com/lattera/glibc/blob/895ef79e04a953cac1493863bcae29ad85657ee1/elf/dl-support.c#L228
    fn init_auxv_stack(&mut self) {
        self.x[SP] -= RANDOM_BYTES;

        let at_random_addr = self.x[SP];

        // initialize random bytes to 0..16
        for i in 0..16 {
            self.memory.store_u8(at_random_addr + i, i as u8);
        }

        let program_name = b"/prog\0";
        self.x[SP] -= 8; // for alignment
        let program_name_addr = self.x[SP];
        self.memory.write_string_n(b"/prog\0", program_name_addr, 8);

        // argc
        self.x[SP] -= 4;
        self.memory.store_u32(self.x[SP], 1); // one argument

        // argv
        self.x[SP] -= 8; // argv[0]
        self.memory.store_u64(self.x[SP], program_name_addr);

        // envp
        self.x[SP] -= 8;

        // minimal auxv
        let aux_values = [
            AuxPair(Auxv::Random, at_random_addr),
            AuxPair(Auxv::Null, 0),
        ];

        for AuxPair(key, val) in aux_values.into_iter() {
            self.x[SP] -= 16;
            log::debug!("Writing {:?}={} at 0x{:x}", key, val, self.x[SP]);
            // self.memory.store_u64(self.x[SP], key as u64);
            self.memory.store_u64(self.x[SP], key as u64);
            self.memory.store_u64(self.x[SP] + 8, val);
        }

        // padding or smthn
        self.x[SP] -= 8;
    }

    // emulates linux syscalls
    fn syscall(&mut self, id: u64) {
        let arg = self.x[A0];

        match id {
            FACCESSAT => {
                self.x[A0] = (-1i64) as u64;
                // TODO: currently just noop (maybe that's fine, who knows)
            }

            WRITE => {
                log::debug!(
                    "Writing to file={}, addr={:x}, nbytes={}",
                    self.x[A0],
                    self.x[A1],
                    self.x[A2]
                );

                let addr = self.x[A1];

                for i in 0..8 {
                    print!("\"{}\" ", self.memory.load_u8(addr + i));
                }

                println!();
            }

            READLINKAT => {
                // let dirfd = self.x[A0];
                let addr = self.x[A1];
                let buf_addr = self.x[A2];
                let bufsize = self.x[A3];

                let s = self.memory.read_string(addr);
                println!(
                    "READLINKAT: {:?} into buffer at 0x{:x}, size={}",
                    s, buf_addr, bufsize
                );

                if s == "/proc/self/exe" {
                    self.memory.write_string_n(b"/prog\0", buf_addr, bufsize);
                } else {
                    panic!("Arbitrary file reading is not supported... YAHHH!");
                }

                self.x[A0] = 0;
            }

            EXIT => {
                self.exit_code = Some(arg);
            }

            EXIT_GROUP => {
                self.exit_code = Some(arg);
            }

            SET_TID_ADDRESS => {
                self.x[A0] = 0;
            }

            SET_ROBUST_LIST => {
                self.x[A0] = 0;
            }

            CLOCK_GETTIME => {
                // noop
            }

            BRK => {
                println!("brk_addr_before={:x}", self.memory.heap_pointer);
                self.x[A0] = self.memory.brk(arg);
                println!("brk_addr_after={:x}", self.memory.heap_pointer);
            }

            MMAP => {
                self.x[A0] = self.memory.mmap(self.x[A1]);
            }

            PRLIMIT64 => {
                self.x[A0] = 0;
            }

            GETRANDOM => {
                let buf = self.x[A0];
                let buflen = self.x[A1];

                // we want this emulator to be deterministic
                for i in buf..(buf + buflen) {
                    self.memory.store_u8(i, 0);
                }
            }

            _ => {
                unimplemented!("syscall {id} not implemented.");
            }
        }
    }

    pub fn fetch_and_execute(&mut self) -> Option<u64> {
        let inst_data = self.memory.load_u32(self.pc);
        let (inst, incr) = Inst::decode(inst_data);

        println!("{:3} {:05x} {}", self.fuel_counter, self.pc, inst);
        self.print_registers();
        // let mut s = String::new();
        // std::io::stdin().read_line(&mut s).ok();

        self.execute(inst, incr as u64);

        self.fuel_counter += 1;
        self.exit_code
    }

    fn execute_raw(&mut self, inst_data: u32) {
        let (inst, incr) = Inst::decode(inst_data);
        self.execute(inst, incr as u64);
        self.print_registers();
    }

    pub fn print_registers(&self) {
        // println!("stack: {:x?}", self.memory.stack);
        // println!("heap_end: {:x?}", self.memory.heap_pointer);
        // println!("dynamic data: {:x?}", self.memory.mmap_regions);
        // println!("fuel consumed: {}", self.fuel_counter);
        for i in 0..32 {
            let reg = Reg(i);
            eprintln!("x{i} ({}):\t{:16x}", reg, self.x[reg]);
        }
    }

    fn execute(&mut self, inst: Inst, incr: u64) {
        match inst {
            Inst::Fence => {} // noop currently, to do with concurrency I think
            Inst::Ecall => {
                let id = self.x[A7];
                self.syscall(id);
            }
            Inst::Error(e) => {
                panic!("{e}");
            }
            Inst::Lui { rd, imm } => {
                self.x[rd] = imm;
            }
            Inst::Ld { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);

                self.x[rd] = self.memory.load_u64(addr);
            }
            Inst::Lw { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u32(addr) as i32 as u64;
            }
            Inst::Lhu { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u16(addr) as u64;
            }
            Inst::Lbu { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u8(addr) as u64;
            }
            Inst::Sd { rs1, rs2, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);

                self.memory.store_u64(addr, self.x[rs2]);
            }
            Inst::Sw { rs1, rs2, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store_u32(addr, self.x[rs2] as u32);
            }
            Inst::Sh { rs1, rs2, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store_u16(addr, self.x[rs2] as u16);
            }
            Inst::Sb { rs1, rs2, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store_u8(addr, self.x[rs2] as u8);
            }
            Inst::Add { rd, rs1, rs2 } => self.x[rd] = self.x[rs1].wrapping_add(self.x[rs2]),
            Inst::Addw { rd, rs1, rs2 } => {
                self.x[rd] = (self.x[rs1] as i32).wrapping_add(self.x[rs2] as i32) as u64;
            }
            Inst::Addi { rd, rs1, imm } => self.x[rd] = self.x[rs1].wrapping_add(imm),
            Inst::Addiw { rd, rs1, imm } => {
                self.x[rd] = (self.x[rs1] as i32).wrapping_add(imm as i32) as u64;
            }
            Inst::And { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] & self.x[rs2];
            }
            Inst::Andi { rd, rs1, imm } => {
                self.x[rd] = self.x[rs1] & imm;
            }
            Inst::Sub { rd, rs1, rs2 } => self.x[rd] = self.x[rs1].wrapping_sub(self.x[rs2]),
            Inst::Subw { rd, rs1, rs2 } => {
                self.x[rd] = (self.x[rs1] as i32).wrapping_sub(self.x[rs2] as i32) as u64;
            }
            Inst::Slli { rd, rs1, shamt } => {
                self.x[rd] = self.x[rs1] << shamt;
            }
            Inst::Slliw { rd, rs1, shamt } => {
                self.x[rd] = ((self.x[rs1] as u32) << shamt) as u64;
            }
            Inst::Srli { rd, rs1, shamt } => {
                self.x[rd] = self.x[rs1] >> shamt;
            }
            Inst::Srliw { rd, rs1, shamt } => {
                self.x[rd] = ((self.x[rs1] as u32) >> shamt) as u64;
            }
            Inst::Or { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] | self.x[rs2];
            }
            Inst::Ori { rd, rs1, imm } => {
                self.x[rd] = self.x[rs1] | imm;
            }
            Inst::Xor { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] ^ self.x[rs2];
            }
            Inst::Xori { rd, rs1, imm } => {
                self.x[rd] = self.x[rs1] ^ imm;
            }
            Inst::Auipc { rd, imm } => {
                self.x[rd] = self.pc.wrapping_add(imm);
            }
            Inst::Jal { rd, offset } => {
                self.x[rd] = self.pc + incr as u64;
                self.pc = self.pc.wrapping_add(offset).wrapping_sub(incr);
            }
            Inst::Jalr { rd, rs1, offset } => {
                self.x[rd] = self.pc + incr as u64;
                self.pc = self.x[rs1].wrapping_add(offset).wrapping_sub(incr);
            }
            Inst::Beq { rs1, rs2, offset } => {
                if self.x[rs1] == self.x[rs2] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bne { rs1, rs2, offset } => {
                if self.x[rs1] != self.x[rs2] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Blt { rs1, rs2, offset } => {
                if (self.x[rs1] as i64) < self.x[rs2] as i64 {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bltu { rs1, rs2, offset } => {
                if self.x[rs1] < self.x[rs2] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Sltu { rd, rs1, rs2 } => {
                if self.x[rs1] < self.x[rs2] {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Sltiu { rd, rs1, imm } => {
                if self.x[rs1] < imm {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Bge { rs1, rs2, offset } => {
                if (self.x[rs1] as i64) >= self.x[rs2] as i64 {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bgeu { rs1, rs2, offset } => {
                if self.x[rs1] >= self.x[rs2] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            // TODO: Divide by zero semantics are NOT correct
            Inst::Divu { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] / self.x[rs2];
            }
            Inst::Mul { rd, rs1, rs2 } => {
                self.x[rd] = (self.x[rs1] as i64 / self.x[rs2] as i64) as u64;
            }
            Inst::Remu { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] % self.x[rs2];
            }
            Inst::Amoswapw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u32(self.x[rs1] as u32 as u64) as i32 as u64;
                let tmp = self.x[rs2] as i32 as u64;
                self.x[rs2] = self.x[rd];
                self.x[rd] = tmp;
                self.memory.store_u32(self.x[rs1], self.x[rd] as u32);
            }
            Inst::Amoswapd { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u64(self.x[rs1]);
                let tmp = self.x[rs2];
                self.x[rs2] = self.x[rd];
                self.x[rd] = tmp;
                self.memory.store_u64(self.x[rs1], self.x[rd]);
            }
        }

        self.pc = self.pc.wrapping_add(incr);

        // make sure x0 is zero
        self.x[0] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test]
    fn lui() {
        let memory = Memory::from_raw(&[]);
        let mut emulator = Emulator::new(0, memory);

        // lui a0, 1000
        emulator.execute_raw(0x003e8537);
        assert_eq!(emulator.x[A0], 4096000);

        // c.lui a0, 10
        emulator.execute_raw(0x000065a9);
        assert_eq!(emulator.x[A1], 40960);
    }

    #[test_log::test]
    fn loads() {
        let memory = Memory::from_raw(&[
            0x12, 0x23, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, //.
            0xef, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, //.
        ]);
        let mut emulator = Emulator::new(0, memory);

        // ld a0, 0(x0)
        emulator.execute_raw(0x00003503);
        assert_eq!(emulator.x[A0], 0xdebc9a7856342312);

        // lw a1, 8(zero)
        emulator.execute_raw(0x00802583);
        assert_eq!(emulator.x[A1], 0xffffffffffffffef);

        // lhu a1, 8(zero)
        emulator.execute_raw(0x00805583);
        assert_eq!(emulator.x[A1], 0x000000000000ffef);

        // lhu a1, 8(zero)
        emulator.execute_raw(0x00804583);
        assert_eq!(emulator.x[A1], 0x00000000000000ef);
    }

    #[test_log::test]
    fn stores() {
        let memory = Memory::from_raw(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //.
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //.
        ]);
        let mut emulator = Emulator::new(0, memory);
        emulator.x[A0] = 0xdebc9a7856342312;

        // sd a0, 0(zero)
        // ld a1, 0(zero)
        emulator.execute_raw(0x00a03023);
        emulator.execute_raw(0x00003583);
        assert_eq!(emulator.x[A0], emulator.x[A1]);

        // -32 2s complement
        emulator.x[A0] = 0xfffffffffffffffe;
        // sw a0, 0(zero)
        // lw a1, 0(zero)
        emulator.execute_raw(0x00a02023);
        emulator.execute_raw(0x00002583);
        assert_eq!(emulator.x[A0], emulator.x[A1]);

        // ld a1, 0(zero)
        emulator.execute_raw(0x00003583);
        assert_ne!(emulator.x[A0], emulator.x[A1]);
    }

    #[test_log::test]
    fn sp_relative() {
        let memory = Memory::from_raw(&[]);
        let mut emulator = Emulator::new(0, memory);
        emulator.x[A0] = 0xdebc9a7856342312;
        let sp_start = emulator.x[SP];

        // C.SDSP a0, 0
        emulator.execute_raw(0x0000e02a);

        // C.LDSP a1, 0
        emulator.execute_raw(0x00006582);
        assert_eq!(emulator.x[A0], emulator.x[A1]);

        // C.ADDI4SPN a0, 8
        emulator.execute_raw(0x00000028);
        assert_eq!(emulator.x[A0], emulator.x[SP] + 8);

        // C.ADDI16SP 32
        emulator.execute_raw(0x00006105);
        assert_eq!(emulator.x[SP], sp_start + 32);

        // C.ADDI16SP -64
        emulator.execute_raw(0x00007139);
        assert_eq!(emulator.x[SP], sp_start - 32);
    }
}
