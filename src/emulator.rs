use std::{
    fmt::Display,
    io::Write,
    ops::{Index, IndexMut},
};

use num_traits::FromPrimitive;

use crate::{
    auxvec::{AuxPair, Auxv, RANDOM_BYTES},
    instruction::Inst,
    memory::Memory,
    syscalls::Syscall,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reg(pub u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FReg(pub u8);

impl Index<Reg> for [u64] {
    type Output = u64;
    fn index(&self, index: Reg) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl IndexMut<Reg> for [u64] {
    fn index_mut(&mut self, index: Reg) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

impl Index<FReg> for [f64] {
    type Output = f64;
    fn index(&self, index: FReg) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl IndexMut<FReg> for [f64] {
    fn index_mut(&mut self, index: FReg) -> &mut Self::Output {
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

impl Display for FReg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            0 => "ft0",
            1 => "ft1",
            2 => "ft2",
            3 => "ft3",
            4 => "ft4",
            5 => "ft5",
            6 => "ft6",
            7 => "ft7",
            8 => "fs0",
            9 => "fs1",
            10 => "fa0",
            11 => "fa1",
            12 => "fa2",
            13 => "fa3",
            14 => "fa4",
            15 => "fa5",
            16 => "fa6",
            17 => "fa7",
            18 => "fs2",
            19 => "fs3",
            20 => "fs4",
            21 => "fs5",
            22 => "fs6",
            23 => "fs7",
            24 => "fs8",
            25 => "fs9",
            26 => "fs10",
            27 => "fs11",
            28 => "ft8",
            29 => "ft9",
            30 => "ft10",
            31 => "ft11",
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

pub struct Emulator {
    pc: u64,
    // fscr: u64,
    x: [u64; 32],
    f: [f64; 32],

    instruction_cache: Option<Box<[(Inst, u8)]>>,
    text_range: (u64, u64),
    memory: Memory,

    exit_code: Option<u64>,

    /// The number of instructions executed over the lifecycle of the emulator.
    pub fuel_counter: u64,
    // Similar to fuel_counter, but also takes into account intruction level parallelism and cache misses.
    // performance_counter: u64,
}

impl Emulator {
    pub fn new(entry: u64, mut memory: Memory) -> Self {
        let mut em = Self {
            pc: entry,
            // fscr: 0,
            x: [0; 32],
            f: [0.0; 32],

            instruction_cache: None,
            text_range: memory.get_text_range(),

            memory,
            exit_code: None,
            fuel_counter: 0,
            // performance_counter: 0,
        };

        em.x[SP] = STACK_START;

        em.init_auxv_stack();

        em
    }

    pub fn precache_instructions(&mut self) {
        let mut instructions = Vec::new();

        // TODO: multithread

        let mut pc = self.text_range.0;
        while pc < self.text_range.1 {
            let inst_data = self.memory.load_u32(pc);
            let inst_with_incr = Inst::decode(inst_data);

            // log::debug!("{pc:07x} {}", inst_with_incr.0);

            pc += inst_with_incr.1 as u64;

            match inst_with_incr.1 {
                2 => instructions.push(inst_with_incr),
                4 => {
                    instructions.push(inst_with_incr);
                    instructions.push(inst_with_incr);
                }
                _ => unreachable!(),
            }
        }

        self.instruction_cache = Some(instructions.into_boxed_slice());
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

        let sc: Syscall = FromPrimitive::from_u64(id).expect(&format!("Unknown syscall: {id}"));

        // self.print_registers();
        log::debug!("Executing syscall {sc:?}");

        match sc {
            Syscall::Faccessat => {
                self.x[A0] = (-1i64) as u64;
                // TODO: currently just noop (maybe that's fine, who knows)
            }

            Syscall::Write => {
                assert!(self.x[A0] <= 2);

                let ptr = self.x[A1];
                let len = self.x[A2];

                log::debug!(
                    "Writing to file={}, addr={:x}, nbytes={}",
                    self.x[A0],
                    self.x[A1],
                    self.x[A2]
                );

                let s = self.memory.read_string_n(ptr, len);
                print!("{s}");

                self.x[A0] = len;
            }

            Syscall::Writev => {
                let fd = self.x[A0];
                assert!(fd <= 2);

                let iovecs = self.x[A1];
                let iovcnt = self.x[A2];

                for i in 0..iovcnt {
                    let ptr = self.memory.load_u64(iovecs + (i * 16));
                    let len = self.memory.load_u64(iovecs + 8 + (i * 16));

                    let s = self.memory.read_string_n(ptr, len);
                    print!("{s}");
                }
            }

            Syscall::Readlinkat => {
                // let dirfd = self.x[A0];
                let addr = self.x[A1];
                let buf_addr = self.x[A2];
                let bufsize = self.x[A3];

                let s = self.memory.read_string(addr);
                // println!(
                //     "READLINKAT: {:?} into buffer at 0x{:x}, size={}",
                //     s, buf_addr, bufsize
                // );

                if s == "/proc/self/exe" {
                    self.memory.write_string_n(b"/prog\0", buf_addr, bufsize);
                    self.x[A0] = 6;
                } else {
                    self.x[A0] = -1i64 as u64;
                    panic!("Arbitrary file reading is not supported... YAHHH!");
                }
            }

            Syscall::Exit => {
                self.exit_code = Some(arg);
            }

            Syscall::ExitGroup => {
                self.exit_code = Some(arg);
            }

            Syscall::SetTidAddress => {
                self.x[A0] = 0;
            }

            Syscall::Futex => {
                self.x[A0] = 0;
                self.memory.store_u64(self.x[A5], 0);
                // self.x[A0] = 0;
            }

            Syscall::SetRobustList => {
                self.x[A0] = 0;
            }

            Syscall::ClockGettime => {
                // noop
            }

            Syscall::Tgkill => {
                self.x[A0] = -1i64 as u64;
            }

            Syscall::RtSigprocmask => {
                self.x[A0] = 0;
            }

            Syscall::Getpid => {
                self.x[A0] = 0;
            }

            Syscall::Gettid => {
                self.x[A0] = 0;
            }

            Syscall::Brk => {
                self.x[A0] = self.memory.brk(arg);
            }

            Syscall::Mmap => {
                self.x[A0] = self.memory.mmap(self.x[A1]);
            }

            Syscall::Mprotect => {
                self.x[A0] = 0;
            }

            Syscall::Prlimit64 => {
                self.x[A0] = 0;
            }

            Syscall::Getrandom => {
                let buf = self.x[A0];
                let buflen = self.x[A1];

                // we want this emulator to be deterministic
                for i in buf..(buf + buflen) {
                    self.memory.store_u8(i, 0xff);
                }

                self.x[A0] = buflen;
            }
            Syscall::Newfstatat => {
                self.x[A0] = 0;
            }
            Syscall::SchedYield => {
                self.x[A0] = 0;
            }
        }
    }

    fn fetch(&mut self) -> (Inst, u8) {
        if let Some(ref cache) = self.instruction_cache {
            cache[(self.pc - self.text_range.0) as usize >> 1]
        } else {
            let inst_data = self.memory.load_u32(self.pc);
            Inst::decode(inst_data)
        }
    }

    pub fn fetch_and_execute(&mut self) -> Option<u64> {
        let (inst, incr) = self.fetch();

        log::debug!("{:3} {:05x} {}", self.fuel_counter, self.pc, inst);

        self.execute(inst, incr as u64);

        self.fuel_counter += 1;
        self.exit_code
    }

    #[cfg(test)]
    fn execute_raw(&mut self, inst_data: u32) {
        let (inst, incr) = Inst::decode(inst_data);
        self.execute(inst, incr as u64);
        self.print_registers();
    }

    pub fn print_registers(&self) {
        for i in 0..32 {
            let reg = Reg(i);
            eprintln!("x{i} ({}):\t{:16x}", reg, self.x[reg]);
        }
    }

    fn execute(&mut self, inst: Inst, incr: u64) {
        match inst {
            Inst::Fence => {} // noop currently, to do with concurrency I think
            Inst::Ebreak => {}
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
            Inst::Fld { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.f[rd] = f64::from_bits(self.memory.load_u64(addr));
            }
            Inst::Lw { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u32(addr) as i32 as u64;
            }
            Inst::Lwu { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u32(addr) as u64;
            }
            Inst::Lhu { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u16(addr) as u64;
            }
            Inst::Lb { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u8(addr) as i8 as u64;
            }
            Inst::Lbu { rd, rs1, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.x[rd] = self.memory.load_u8(addr) as u64;
            }
            Inst::Sd { rs1, rs2, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store_u64(addr, self.x[rs2]);
            }
            Inst::Fsd { rs1, rs2, offset } => {
                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store_u64(addr, self.f[rs2].to_bits());
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
            Inst::Sll { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] << self.x[rs2];
            }
            Inst::Sllw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as u32).wrapping_shl(self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Slli { rd, rs1, shamt } => {
                self.x[rd] = self.x[rs1] << shamt;
            }
            Inst::Slliw { rd, rs1, shamt } => {
                self.x[rd] = ((self.x[rs1] as u32).wrapping_shl(shamt)) as u64;
            }
            Inst::Srl { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1].wrapping_shl(self.x[rs2] as u32);
            }
            Inst::Srlw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as u32).wrapping_shr(self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Srli { rd, rs1, shamt } => {
                self.x[rd] = self.x[rs1] >> shamt;
            }
            Inst::Srliw { rd, rs1, shamt } => {
                self.x[rd] = ((self.x[rs1] as u32).wrapping_shr(shamt)) as u64;
            }
            Inst::Sra { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as i64).wrapping_shr(self.x[rs2] as u32)) as u64;
            }
            Inst::Sraw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as i32).wrapping_shr(self.x[rs2] as u32)) as u64;
            }
            Inst::Srai { rd, rs1, shamt } => {
                self.x[rd] = ((self.x[rs1] as i64) >> shamt) as u64;
            }
            Inst::Sraiw { rd, rs1, shamt } => {
                self.x[rd] = ((self.x[rs1] as i32) >> shamt) as u64;
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
            Inst::Slt { rd, rs1, rs2 } => {
                if (self.x[rs1] as i64) < (self.x[rs2] as i64) {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Sltu { rd, rs1, rs2 } => {
                if self.x[rs1] < self.x[rs2] {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Slti { rd, rs1, imm } => {
                if (self.x[rs1] as i64) < (imm as i64) {
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
            Inst::Div { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as i64) / (self.x[rs2] as i64)) as u64;
            }
            Inst::Divw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as i32) / (self.x[rs2] as i32)) as u64;
            }
            Inst::Divu { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] / self.x[rs2];
            }
            Inst::Divuw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as u32) / (self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Mul { rd, rs1, rs2 } => {
                self.x[rd] = (self.x[rs1] as i64).wrapping_mul(self.x[rs2] as i64) as u64;
            }
            Inst::Mulhu { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as u128).wrapping_mul(self.x[rs2] as u128) >> 64) as u64;
            }
            Inst::Remw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as i32) % (self.x[rs2] as i32)) as u64;
            }
            Inst::Remu { rd, rs1, rs2 } => {
                self.x[rd] = self.x[rs1] % self.x[rs2];
            }
            Inst::Remuw { rd, rs1, rs2 } => {
                self.x[rd] = ((self.x[rs1] as u32) % (self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Amoswapw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u32(self.x[rs1]) as i32 as u64;
                self.memory.store_u32(self.x[rs1], self.x[rs2] as u32);
            }
            Inst::Amoswapd { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u64(self.x[rs1]);
                self.memory.store_u64(self.x[rs1], self.x[rs2]);
            }
            Inst::Amoaddw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u32(self.x[rs1]) as i32 as u64;
                self.memory.store_u32(
                    self.x[rs1],
                    (self.x[rs2] as u32).wrapping_add(self.x[rd] as u32),
                );
            }
            Inst::Amoaddd { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u64(self.x[rs1]);
                self.memory
                    .store_u64(self.x[rs1], self.x[rs2].wrapping_add(self.x[rd]));
            }
            Inst::Amoorw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u32(self.x[rs1]) as i32 as u64;
                self.memory
                    .store_u32(self.x[rs1], (self.x[rs2] as u32) | (self.x[rd] as u32));
            }
            Inst::Amomaxuw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u32(self.x[rs1]) as i32 as u64;
                self.memory
                    .store_u32(self.x[rs1], (self.x[rs2] as u32).max(self.x[rd] as u32));
            }
            Inst::Amomaxud { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load_u64(self.x[rs1]);
                self.memory
                    .store_u64(self.x[rs1], self.x[rs2].max(self.x[rd]));
            }
            Inst::Lrw { rd, rs1 } => {
                self.x[rd] = self.memory.load_u32(self.x[rs1]) as i32 as u64;
            }
            Inst::Lrd { rd, rs1 } => {
                self.x[rd] = self.memory.load_u64(self.x[rs1]);
            }
            Inst::Scw { rd, rs1, rs2 } => {
                self.x[rd] = 0;
                self.memory.store_u32(self.x[rs1], self.x[rs2] as u32);
            }
            Inst::Scd { rd, rs1, rs2 } => {
                self.x[rd] = 0;
                self.memory.store_u64(self.x[rs1], self.x[rs2]);
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

    // #[test_log::test]
    // fn stores() {
    //     let memory = Memory::from_raw(&[
    //         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //.
    //         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //.
    //     ]);
    //     let mut emulator = Emulator::new(0, memory);
    //     emulator.x[A0] = 0xdebc9a7856342312;
    //
    //     // sd a0, 0(zero)
    //     // ld a1, 0(zero)
    //     emulator.execute_raw(0x00a03023);
    //     emulator.execute_raw(0x00003583);
    //     assert_eq!(emulator.x[A0], emulator.x[A1]);
    //
    //     // -32 2s complement
    //     emulator.x[A0] = 0xfffffffffffffffe;
    //     // sw a0, 0(zero)
    //     // lw a1, 0(zero)
    //     emulator.execute_raw(0x00a02023);
    //     emulator.execute_raw(0x00002583);
    //     assert_eq!(emulator.x[A0], emulator.x[A1]);
    //
    //     // ld a1, 0(zero)
    //     emulator.execute_raw(0x00003583);
    //     assert_ne!(emulator.x[A0], emulator.x[A1]);
    // }

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
