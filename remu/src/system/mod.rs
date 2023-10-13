use std::{
    collections::{BTreeMap, HashMap},
    num::NonZeroU64,
    path::Path,
    rc::Rc,
};

use elf::{endian::AnyEndian, ElfBytes};

use crate::{
    auxvec::{AuxPair, Auxv, RANDOM_BYTES},
    error::RVError,
    files::FileDescriptor,
    instruction::Inst,
    memory::{Memory, PAGE_SIZE},
    profiler::Profiler,
    register::*,
};

use self::jit::RVFunction;

mod interp;
mod jit;
mod syscall;

pub const STACK_START: u64 = -1i64 as u64;

// https://sifive.cdn.prismic.io/sifive/1a82e600-1f93-4f41-b2d8-86ed8b16acba_fu740-c000-manual-v1p6.pdf
// The latency of DIV, DIVU, REM, and REMU instructions can be determined by calculating:
// Latency = 2 cycles + log2(dividend) - log2(divisor) + 1 cycle
// if the input is negative + 1 cycle if the output is negative
macro_rules! div_cycle_count {
    ($dividend:expr, $divisor:expr) => {
        (2 + ($dividend)
            .max(1)
            .ilog2()
            .saturating_sub(($divisor).max(1).ilog2())) as u64
    };
}

#[derive(Clone)]
pub struct Emulator {
    pub pc: u64,
    // fscr: u64,
    x: [u64; 32],
    f: [f64; 32],

    pub memory: Memory,
    file_descriptors: HashMap<i64, FileDescriptor>,

    pub stdout: String,
    pub stderr: String,

    profile_start_point: Option<NonZeroU64>,
    profile_end_point: Option<NonZeroU64>,
    pub profiler: Profiler,

    /// The number of instructions executed over the lifecycle of the emulator.
    pub inst_counter: u64,
    pub max_memory: u64,

    jit_functions: BTreeMap<u64, Rc<RVFunction>>,

    // Similar to fuel_counter, but also takes into account intruction level parallelism and cache misses.
    // performance_counter: u64,
    pub exit_code: Option<u64>,
}

impl Emulator {
    pub fn new(memory: Memory) -> Self {
        let mut em = Self {
            pc: memory.entry,
            // fscr: 0,
            x: [0; 32],
            f: [0.0; 32],

            file_descriptors: HashMap::default(),
            stdout: String::new(),
            stderr: String::new(),

            // if set, only count cycles when profile_start_point
            // then stop when return profile_end_point is reached
            // (automatically set from RA when profile_start_point is reached)
            profile_start_point: None,
            profile_end_point: None,
            profiler: Profiler::new(),

            jit_functions: BTreeMap::new(),

            memory,
            exit_code: None,
            inst_counter: 0,
            max_memory: 0,
        };

        em.x[SP] = STACK_START;

        // this can never fail
        em.init_auxv_stack()
            .expect("Failed to initialize aux vector");

        em
    }

    pub fn from_file<P>(path: P) -> Result<Emulator, anyhow::Error>
    where
        P: AsRef<Path>,
    {
        let file_data = std::fs::read(path).expect("Could not read file.");
        let slice = file_data.as_slice();
        let file = ElfBytes::<AnyEndian>::minimal_parse(slice)?;

        match (file.ehdr.class, file.ehdr.e_type, file.ehdr.e_machine) {
            // (64 bit, executable, risc_v arch)
            (elf::file::Class::ELF64, 0x03 | 0x02, 0xF3) => log::info!("Parsing executable."),
            _ => return Err(RVError::InvalidFileType.into()),
        }

        let memory = Memory::load_elf(file);
        let emulator = Emulator::new(memory);

        Ok(emulator)
    }

    pub fn profile_label(&mut self, label: &str) -> Result<(), RVError> {
        self.profile_start_point = NonZeroU64::new(
            self.memory
                .disassembler
                .get_symbol_addr(label)
                .ok_or(RVError::InvalidLabel)?,
        );

        Ok(())
    }

    pub fn set_stdin(&mut self, data: &[u8]) {
        self.file_descriptors.insert(
            0,
            FileDescriptor {
                offset: 0,
                data: data.into(),
            },
        );
    }

    // https://github.com/torvalds/linux/blob/master/fs/binfmt_elf.c#L175
    // https://github.com/lattera/glibc/blob/895ef79e04a953cac1493863bcae29ad85657ee1/elf/dl-support.c#L228
    fn init_auxv_stack(&mut self) -> Result<(), RVError> {
        self.x[SP] -= RANDOM_BYTES;

        let at_random_addr = self.x[SP];

        // initialize random bytes to 0..16
        for i in 0..16 {
            self.memory.store::<u8>(at_random_addr + i, i as u8)?;
        }

        self.x[SP] -= 8; // for alignment
        let program_name_addr = self.x[SP];
        self.memory.write_n(b"/prog\0", program_name_addr, 8)?;

        self.x[SP] -= 16;
        let envp1_addr = self.x[SP];
        self.memory.write_n(b"LD_DEBUG=all\0", envp1_addr, 13)?;

        // argc
        self.x[SP] -= 8;
        self.memory.store(self.x[SP], 1u32)?; // one argument

        // argv
        self.x[SP] -= 8; // argv[0]
        self.memory.store(self.x[SP], program_name_addr)?;

        log::trace!("Writing argv to addr=0x{:x}", self.x[SP]);

        // envp
        // self.x[SP] -= 8; // envp[0]
        // self.memory.store_u64(self.x[SP], envp1_addr);
        self.x[SP] -= 8;

        // minimal auxv
        let aux_values = [
            AuxPair(Auxv::Entry, self.memory.program_header.entry), // The address of the entry of the executable
            AuxPair(Auxv::Phdr, self.memory.program_header.address), // The address of the program header of the executable
            AuxPair(Auxv::Phent, self.memory.program_header.size), // The size of the program header entry
            AuxPair(Auxv::Phnum, self.memory.program_header.number), // The number of the program headers
            AuxPair(Auxv::Uid, 0),
            AuxPair(Auxv::Euid, 0),
            AuxPair(Auxv::Gid, 0),
            AuxPair(Auxv::Egid, 0),
            AuxPair(Auxv::Secure, 0),
            AuxPair(Auxv::Pagesz, PAGE_SIZE),
            AuxPair(Auxv::Random, at_random_addr),
            AuxPair(Auxv::Execfn, program_name_addr),
            AuxPair(Auxv::Null, 0),
        ];

        for AuxPair(key, val) in aux_values.into_iter() {
            self.x[SP] -= 16;
            log::trace!("Writing {:?}=0x{:x} at 0x{:x}", key, val, self.x[SP]);
            // self.memory.store_u64(self.x[SP], key as u64);
            self.memory.store(self.x[SP], key as u64)?;
            self.memory.store(self.x[SP] + 8, val)?;
        }

        // padding or smthn
        self.x[SP] -= 8;

        Ok(())
    }

    pub fn fetch(&self) -> Result<(Inst, u8), RVError> {
        let inst_data = self.memory.load::<u32>(self.pc)?;
        Ok(Inst::decode(inst_data))
    }

    fn execute_block(&mut self) -> Result<Option<u64>, RVError> {
        if let Some(stored) = self.jit_functions.get(&self.pc) {
            stored.clone().run(self);
        } else {
            let profile = self.profile_start_point.is_some();
            let newfunc = Rc::new(RVFunction::compile(self, profile));
            self.jit_functions.insert(self.pc, newfunc.clone());
            newfunc.run(self);
        }

        Ok(self.exit_code)
    }

    pub fn run(&mut self, jit: bool) -> Result<u64, RVError> {
        if jit {
            // jit
            loop {
                if let Some(exit_code) = self.execute_block()? {
                    return Ok(exit_code);
                }
            }
        } else {
            // interp
            loop {
                if let Some(exit_code) = self.fetch_and_execute()? {
                    return Ok(exit_code);
                }
            }
        }
    }

    pub fn fetch_and_execute(&mut self) -> Result<Option<u64>, RVError> {
        if self.exit_code.is_some() {
            return Ok(self.exit_code);
        }

        let (inst, incr) = self.fetch()?;

        // if we reach the end
        if NonZeroU64::new(self.pc) == self.profile_start_point {
            self.profile_end_point = NonZeroU64::new(self.x[RA]);
            self.profiler.running = true;
        }
        // save final_cycle_count
        else if NonZeroU64::new(self.pc) == self.profile_end_point {
            self.profile_start_point = None;
            self.profile_end_point = None;
            self.profiler.running = false;
        }

        // this log statement is nice but it is super slow even when not printing unfortunately
        // log::debug!("{:16x} {}", self.pc, inst.fmt(self.pc));

        self.execute(inst, incr as u64)?;

        self.max_memory = self.max_memory.max(self.memory.usage());

        Ok(self.exit_code)
    }

    #[cfg(test)]
    fn execute_raw(&mut self, inst_data: u32) -> Result<(), RVError> {
        let (inst, incr) = Inst::decode(inst_data);
        self.execute(inst, incr as u64)?;
        self.print_registers();

        Ok(())
    }

    pub fn print_registers(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("pc: {:22x}\n", self.pc));
        output.push_str(&format!("fuel cnt: {:16}\n", self.inst_counter));

        for i in 0..32 {
            let reg = Reg(i);
            let start = format!("x{i} ({}):", reg);
            output.push_str(&format!("{start:10}{:16x}\n", self.x[reg]));
        }

        output
    }

    fn execute(&mut self, inst: Inst, incr: u64) -> Result<(), RVError> {
        match inst {
            Inst::Fence => {} // noop currently, to do with concurrency I think
            Inst::Ebreak => {}
            Inst::Ecall => {
                self.profiler.pipeline_stall_x(A7, self.pc);

                self.syscall()?;
            }
            Inst::Error(e) => {
                log::error!("unknown instruction: {e:x}");
            }
            Inst::Lui { rd, imm } => {
                self.x[rd] = imm as u64;
            }
            Inst::Ld { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_x(rd, addr, self.pc);

                self.x[rd] = self.memory.load(addr)?;
            }
            Inst::Fld { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_f(rd, addr, self.pc);

                self.f[rd] = f64::from_bits(self.memory.load(addr)?);
            }
            Inst::Flw { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_f(rd, addr, self.pc);

                self.f[rd] = f32::from_bits(self.memory.load(addr)?) as f64;
            }
            Inst::Lw { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_x(rd, addr, self.pc);

                self.x[rd] = self.memory.load::<i32>(addr)? as u64;
            }
            Inst::Lwu { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_x(rd, addr, self.pc);

                self.x[rd] = self.memory.load::<u32>(addr)? as u64;
            }
            Inst::Lhu { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_x(rd, addr, self.pc);

                self.x[rd] = self.memory.load::<u16>(addr)? as u64;
            }
            Inst::Lb { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_x(rd, addr, self.pc);

                self.x[rd] = self.memory.load::<i8>(addr)? as u64;
            }
            Inst::Lbu { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.profiler.add_load_delay_x(rd, addr, self.pc);

                self.x[rd] = self.memory.load::<u8>(addr)? as u64;
            }
            Inst::Sd { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store(addr, self.x[rs2])?;
            }
            Inst::Fsd { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xf(rs1, rs2, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store(addr, self.f[rs2].to_bits())?;
            }
            Inst::Fsw { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xf(rs1, rs2, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store(addr, (self.f[rs2] as f32).to_bits())?;
            }
            Inst::Sw { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store(addr, self.x[rs2] as u32)?;
            }
            Inst::Sh { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store(addr, self.x[rs2] as u16)?;
            }
            Inst::Sb { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                let addr = self.x[rs1].wrapping_add(offset as u64);
                self.memory.store(addr, self.x[rs2] as u8)?;
            }
            Inst::Add { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1].wrapping_add(self.x[rs2]);
            }
            Inst::Addw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = (self.x[rs1] as i32).wrapping_add(self.x[rs2] as i32) as u64;
            }
            Inst::Addi { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.x[rs1].wrapping_add(imm as u64);
            }
            Inst::Addiw { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = (self.x[rs1] as i32).wrapping_add(imm) as u64;
            }
            Inst::And { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1] & self.x[rs2];
            }
            Inst::Andi { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.x[rs1] & (imm as u64);
            }
            Inst::Sub { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1].wrapping_sub(self.x[rs2]);
            }
            Inst::Subw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = (self.x[rs1] as i32).wrapping_sub(self.x[rs2] as i32) as u64;
            }
            Inst::Sll { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1] << self.x[rs2];
            }
            Inst::Sllw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = ((self.x[rs1] as u32).wrapping_shl(self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Slli { rd, rs1, shamt } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.x[rs1] << shamt;
            }
            Inst::Slliw { rd, rs1, shamt } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = ((self.x[rs1] as u32).wrapping_shl(shamt)) as u64;
            }
            Inst::Srl { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1].wrapping_shr(self.x[rs2] as u32);
            }
            Inst::Srlw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = ((self.x[rs1] as u32).wrapping_shr(self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Srli { rd, rs1, shamt } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.x[rs1] >> shamt;
            }
            Inst::Srliw { rd, rs1, shamt } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = ((self.x[rs1] as u32).wrapping_shr(shamt)) as u64;
            }
            Inst::Sra { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = ((self.x[rs1] as i64).wrapping_shr(self.x[rs2] as u32)) as u64;
            }
            Inst::Sraw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = ((self.x[rs1] as i32).wrapping_shr(self.x[rs2] as u32)) as u64;
            }
            Inst::Srai { rd, rs1, shamt } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = ((self.x[rs1] as i64) >> shamt) as u64;
            }
            Inst::Sraiw { rd, rs1, shamt } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = ((self.x[rs1] as i32) >> shamt) as u64;
            }
            Inst::Or { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1] | self.x[rs2];
            }
            Inst::Ori { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.x[rs1] | imm as u64;
            }
            Inst::Xor { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                self.x[rd] = self.x[rs1] ^ self.x[rs2];
            }
            Inst::Xori { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.x[rs1] ^ imm as u64;
            }
            Inst::Auipc { rd, imm } => {
                self.x[rd] = self.pc.wrapping_add(imm as i64 as u64);
            }
            Inst::Jal { rd, offset } => {
                self.x[rd] = self.pc + incr as u64;
                self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
            }
            Inst::Jalr { rd, rs1, offset } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                self.x[rd] = self.pc + incr as u64;
                self.pc = self.x[rs1].wrapping_add(offset as u64).wrapping_sub(incr);
            }
            Inst::Beq { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if self.x[rs1] == self.x[rs2] {
                    self.profiler.branch_taken(self.pc);

                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                } else {
                    self.profiler.branch_not_taken(self.pc);
                }
            }
            Inst::Bne { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if self.x[rs1] != self.x[rs2] {
                    self.profiler.branch_taken(self.pc);

                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                } else {
                    self.profiler.branch_not_taken(self.pc);
                }
            }
            Inst::Blt { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if (self.x[rs1] as i64) < self.x[rs2] as i64 {
                    self.profiler.branch_taken(self.pc);

                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                } else {
                    self.profiler.branch_not_taken(self.pc);
                }
            }
            Inst::Bltu { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if self.x[rs1] < self.x[rs2] {
                    self.profiler.branch_taken(self.pc);

                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                } else {
                    self.profiler.branch_not_taken(self.pc);
                }
            }
            Inst::Slt { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if (self.x[rs1] as i64) < (self.x[rs2] as i64) {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Sltu { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if self.x[rs1] < self.x[rs2] {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Slti { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                if (self.x[rs1] as i64) < (imm as i64) {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Sltiu { rd, rs1, imm } => {
                self.profiler.pipeline_stall_x(rs1, self.pc);

                if self.x[rs1] < imm as u64 {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Bge { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if (self.x[rs1] as i64) >= self.x[rs2] as i64 {
                    self.profiler.branch_taken(self.pc);

                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                } else {
                    self.profiler.branch_not_taken(self.pc);
                }
            }
            Inst::Bgeu { rs1, rs2, offset } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);

                if self.x[rs1] >= self.x[rs2] {
                    self.profiler.branch_taken(self.pc);

                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                } else {
                    self.profiler.branch_not_taken(self.pc);
                }
            }
            // TODO: Divide by zero semantics are NOT correct
            Inst::Div { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler.add_delay_x(
                    rd,
                    div_cycle_count!((self.x[rs1] as i64).abs(), (self.x[rs2] as i64).abs()),
                );

                self.x[rd] = ((self.x[rs1] as i64) / (self.x[rs2] as i64)) as u64;
            }
            Inst::Divw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler.add_delay_x(
                    rd,
                    div_cycle_count!((self.x[rs1] as i32).abs(), (self.x[rs2] as i32).abs()),
                );

                self.x[rd] = ((self.x[rs1] as i32) / (self.x[rs2] as i32)) as u64;
            }
            Inst::Divu { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler
                    .add_delay_x(rd, div_cycle_count!(self.x[rs1], self.x[rs2]));

                self.x[rd] = self.x[rs1] / self.x[rs2];
            }
            Inst::Divuw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler
                    .add_delay_x(rd, div_cycle_count!(self.x[rs1] as u32, self.x[rs2] as u32));

                self.x[rd] = ((self.x[rs1] as u32) / (self.x[rs2] as u32)) as i32 as u64;
            }
            Inst::Mul { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler.add_delay_x(rd, 3);

                self.x[rd] = (self.x[rs1] as i64).wrapping_mul(self.x[rs2] as i64) as u64;
            }
            Inst::Mulhu { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler.add_delay_x(rd, 3);

                self.x[rd] = ((self.x[rs1] as u128).wrapping_mul(self.x[rs2] as u128) >> 64) as u64;
            }
            Inst::Remw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler.add_delay_x(
                    rd,
                    div_cycle_count!((self.x[rs1] as i32).abs(), (self.x[rs2] as i32).abs()),
                );

                if self.x[rs2] == 0 {
                    self.x[rd] = (self.x[rs1] as i32) as u64;
                } else {
                    self.x[rd] = ((self.x[rs1] as i32) % (self.x[rs2] as i32)) as u64;
                }
            }
            Inst::Remu { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler
                    .add_delay_x(rd, div_cycle_count!(self.x[rs1], self.x[rs2]));

                if self.x[rs2] == 0 {
                    self.x[rd] = self.x[rs1];
                } else {
                    self.x[rd] = self.x[rs1] % self.x[rs2];
                }
            }
            Inst::Remuw { rd, rs1, rs2 } => {
                self.profiler.pipeline_stall_xx(rs1, rs2, self.pc);
                self.profiler
                    .add_delay_x(rd, div_cycle_count!(self.x[rs1] as u32, self.x[rs2] as u32));

                if self.x[rs2] == 0 {
                    self.x[rd] = self.x[rs1] as u32 as u64;
                } else {
                    self.x[rd] = ((self.x[rs1] as u32) % (self.x[rs2] as u32)) as i32 as u64;
                }
            }
            Inst::Amoswapw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load::<i32>(self.x[rs1])? as u64;
                self.memory.store(self.x[rs1], self.x[rs2] as u32)?;
            }
            Inst::Amoswapd { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load(self.x[rs1])?;
                self.memory.store(self.x[rs1], self.x[rs2])?;
            }
            Inst::Amoaddw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load::<i32>(self.x[rs1])? as u64;
                self.memory.store(
                    self.x[rs1],
                    (self.x[rs2] as u32).wrapping_add(self.x[rd] as u32),
                )?;
            }
            Inst::Amoaddd { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load(self.x[rs1])?;
                self.memory
                    .store(self.x[rs1], self.x[rs2].wrapping_add(self.x[rd]))?;
            }
            Inst::Amoorw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load::<i32>(self.x[rs1])? as u64;
                self.memory
                    .store(self.x[rs1], (self.x[rs2] as u32) | (self.x[rd] as u32))?;
            }
            Inst::Amomaxuw { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load::<i32>(self.x[rs1])? as u64;
                self.memory
                    .store(self.x[rs1], (self.x[rs2] as u32).max(self.x[rd] as u32))?;
            }
            Inst::Amomaxud { rd, rs1, rs2 } => {
                self.x[rd] = self.memory.load(self.x[rs1])?;
                self.memory
                    .store(self.x[rs1], self.x[rs2].max(self.x[rd]))?;
            }
            Inst::Lrw { rd, rs1 } => {
                self.x[rd] = self.memory.load::<i32>(self.x[rs1])? as u64;
            }
            Inst::Lrd { rd, rs1 } => {
                self.x[rd] = self.memory.load(self.x[rs1])?;
            }
            Inst::Scw { rd, rs1, rs2 } => {
                self.x[rd] = 0;
                self.memory.store(self.x[rs1], self.x[rs2] as u32)?;
            }
            Inst::Scd { rd, rs1, rs2 } => {
                self.x[rd] = 0;
                self.memory.store(self.x[rs1], self.x[rs2])?;
            }
            Inst::Fcvtdlu { rd, rs1, rm: _rm } => {
                // ignore rounding mode for now, super incorrect
                // TODO: fix
                self.x[rd] = self.f[rs1] as u64;
            }
            Inst::Fcvtds { rd, rs1, rm: _rm } => {
                // ignore rounding mode for now, super incorrect
                // TODO: fix
                self.x[rd] = self.f[rs1] as u64;
            }
            Inst::Fled { rd, rs1, rs2 } => {
                if self.f[rs1] < self.f[rs2] {
                    self.x[rd] = 1;
                } else {
                    self.x[rd] = 0;
                }
            }
            Inst::Fdivd { rd, rs1, rs2 } => {
                self.f[rd] = self.f[rs1] / self.f[rs2];
            }
        }

        self.pc = self.pc.wrapping_add(incr);

        self.inst_counter += 1;
        self.profiler.tick(self.pc);

        // make sure x0 is zero
        self.x[0] = 0;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lui() -> Result<(), RVError> {
        let memory = Memory::from_raw(&[]);
        let mut emulator = Emulator::new(memory);

        // lui a0, 1000
        emulator.execute_raw(0x003e8537)?;
        assert_eq!(emulator.x[A0], 4096000);

        // c.lui a0, 10
        emulator.execute_raw(0x000065a9)?;
        assert_eq!(emulator.x[A1], 40960);

        Ok(())
    }

    #[test]
    fn loads() -> Result<(), RVError> {
        let memory = Memory::from_raw(&[
            0x12, 0x23, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, //.
            0xef, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, //.
        ]);
        let mut emulator = Emulator::new(memory);

        // ld a0, 0(x0)
        emulator.execute_raw(0x00003503)?;
        assert_eq!(emulator.x[A0], 0xdebc9a7856342312);

        // lw a1, 8(zero)
        emulator.execute_raw(0x00802583)?;
        assert_eq!(emulator.x[A1], 0xffffffffffffffef);

        // lhu a1, 8(zero)
        emulator.execute_raw(0x00805583)?;
        assert_eq!(emulator.x[A1], 0x000000000000ffef);

        // lhu a1, 8(zero)
        emulator.execute_raw(0x00804583)?;
        assert_eq!(emulator.x[A1], 0x00000000000000ef);

        Ok(())
    }

    #[test]
    fn stores() -> Result<(), RVError> {
        let memory = Memory::from_raw(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //.
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //.
        ]);
        let mut emulator = Emulator::new(memory);
        emulator.x[A0] = 0xdebc9a7856342312;

        // sd a0, 0(zero)
        // ld a1, 0(zero)
        emulator.execute_raw(0x00a03023)?;
        emulator.execute_raw(0x00003583)?;
        assert_eq!(emulator.x[A0], emulator.x[A1]);

        // -32 2s complement
        emulator.x[A0] = 0xfffffffffffffffe;
        // sw a0, 0(zero)
        // lw a1, 0(zero)
        emulator.execute_raw(0x00a02023)?;
        emulator.execute_raw(0x00002583)?;
        assert_eq!(emulator.x[A0], emulator.x[A1]);

        // ld a1, 0(zero)
        emulator.execute_raw(0x00003583)?;
        assert_ne!(emulator.x[A0], emulator.x[A1]);

        Ok(())
    }

    #[test]
    fn sp_relative() -> Result<(), RVError> {
        let memory = Memory::from_raw(&[]);
        let mut emulator = Emulator::new(memory);
        emulator.x[A0] = 0xdebc9a7856342312;
        let sp_start = emulator.x[SP];

        // C.SDSP a0, 0
        emulator.execute_raw(0x0000e02a)?;

        // C.LDSP a1, 0
        emulator.execute_raw(0x00006582)?;
        assert_eq!(emulator.x[A0], emulator.x[A1]);

        // C.ADDI4SPN a0, 8
        emulator.execute_raw(0x00000028)?;
        assert_eq!(emulator.x[A0], emulator.x[SP] + 8);

        // C.ADDI16SP 32
        emulator.execute_raw(0x00006105)?;
        assert_eq!(emulator.x[SP], sp_start + 32);

        // C.ADDI16SP -64
        emulator.execute_raw(0x00007139)?;
        assert_eq!(emulator.x[SP], sp_start - 32);

        Ok(())
    }
}
