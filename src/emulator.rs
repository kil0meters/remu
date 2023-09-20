use crate::{instruction::Inst, memory::Memory};

pub const SP: usize = 2;
pub const A0: usize = 10;
pub const A1: usize = 11;
pub const A2: usize = 12;
pub const A3: usize = 13;
pub const A4: usize = 14;
pub const A5: usize = 15;
pub const A6: usize = 16;
pub const A7: usize = 17;

pub const STACK_START: u64 = 0x8000000000000000;

pub struct Emulator {
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
    pub fn new(entry: u64, memory: Memory) -> Self {
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

    pub fn fetch_and_execute(&mut self) -> Option<u64> {
        let inst_data = self.memory.load_u32(self.pc);
        let (inst, incr) = Inst::decode(inst_data);

        self.print_registers();
        self.execute(inst, incr as u64);

        // let mut res = String::new();
        // std::io::stdin().read_line(&mut res).unwrap();

        self.fuel_counter += 1;
        self.exit_code
    }

    pub fn print_registers(&self) {
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

    fn execute(&mut self, inst: Inst, incr: u64) {
        log::debug!("{:05x} {:x?}", self.pc, inst);

        match inst {
            Inst::Fence => {} // noop currently
            Inst::Ecall => {
                let id = self.reg[A7];
                self.syscall(id);
            }
            Inst::Error(e) => {
                panic!("{e}");
            }
            Inst::Lui { rd, imm } => {
                self.reg[rd as usize] = imm as u64;
            }
            Inst::Ld { rd, rs1, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.reg[rd as usize] = self.memory.load_u64(addr);
            }
            Inst::Lw { rd, rs1, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.reg[rd as usize] = self.memory.load_u32(addr) as i32 as u64;
            }
            Inst::Lhu { rd, rs1, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.reg[rd as usize] = self.memory.load_u16(addr) as i16 as u64;
            }
            Inst::Lbu { rd, rs1, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.reg[rd as usize] = self.memory.load_u16(addr) as i32 as u64;
            }
            Inst::Sd { rs1, rs2, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.memory.store_u64(addr, self.reg[rs2 as usize]);
            }
            Inst::Sw { rs1, rs2, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.memory.store_u32(addr, self.reg[rs2 as usize] as u32);
            }
            Inst::Sh { rs1, rs2, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.memory.store_u16(addr, self.reg[rs2 as usize] as u16);
            }
            Inst::Sb { rs1, rs2, offset } => {
                let addr = self.reg[rs1 as usize].wrapping_add(offset as u64);
                self.memory.store_u8(addr, self.reg[rs2 as usize] as u8);
            }
            Inst::Add { rd, rs1, rs2 } => {
                self.reg[rd as usize] = self.reg[rs1 as usize].wrapping_add(self.reg[rs2 as usize])
            }
            Inst::Addw { rd, rs1, rs2 } => {
                self.reg[rd as usize] = (self.reg[rs1 as usize] as u32)
                    .wrapping_add(self.reg[rs2 as usize] as u32)
                    as i32 as u64;
            }
            Inst::Addi { rd, rs1, imm } => {
                self.reg[rd as usize] = self.reg[rs1 as usize].wrapping_add(imm)
            }
            Inst::Addiw { rd, rs1, imm } => {
                self.reg[rd as usize] =
                    (self.reg[rs1 as usize] as u32).wrapping_add(imm) as i32 as u64;
            }
            Inst::And { rd, rs1, rs2 } => {
                self.reg[rd as usize] = self.reg[rs1 as usize] & self.reg[rs2 as usize];
            }
            Inst::Andi { rd, rs1, imm } => {
                self.reg[rd as usize] = self.reg[rs1 as usize] & imm;
            }
            Inst::Sub { rd, rs1, rs2 } => {
                self.reg[rd as usize] = self.reg[rs1 as usize].wrapping_sub(self.reg[rs2 as usize])
            }
            Inst::Subw { rd, rs1, rs2 } => {
                self.reg[rd as usize] = (self.reg[rs1 as usize] as u32)
                    .wrapping_sub(self.reg[rs2 as usize] as u32)
                    as i32 as u64;
            }
            Inst::Slli { rd, rs1, shamt } => {
                self.reg[rd as usize] = self.reg[rs1 as usize] << shamt;
            }
            Inst::Slliw { rd, rs1, shamt } => {
                self.reg[rd as usize] = ((self.reg[rs1 as usize] as u32) << shamt) as u64;
            }
            Inst::Srli { rd, rs1, shamt } => {
                self.reg[rd as usize] = self.reg[rs1 as usize] >> shamt;
            }
            Inst::Or { rd, rs1, rs2 } => {
                self.reg[rd as usize] = self.reg[rs1 as usize] | self.reg[rs2 as usize];
            }
            Inst::Xor { rd, rs1, rs2 } => {
                self.reg[rd as usize] = self.reg[rs1 as usize] ^ self.reg[rs2 as usize];
            }
            Inst::Auipc { rd, imm } => {
                self.reg[rd as usize] = self.pc.wrapping_add(imm);
            }
            Inst::Jal { rd, offset } => {
                self.reg[rd as usize] = self.pc + incr as u64;
                self.pc = self.pc.wrapping_add(offset).wrapping_sub(incr);
            }
            Inst::Jalr { rd, rs1, offset } => {
                self.reg[rd as usize] = self.pc + incr as u64;
                self.pc = self.reg[rs1 as usize]
                    .wrapping_add(offset)
                    .wrapping_sub(incr);
            }
            Inst::Beq { rs1, rs2, offset } => {
                if self.reg[rs1 as usize] == self.reg[rs2 as usize] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bne { rs1, rs2, offset } => {
                if self.reg[rs1 as usize] != self.reg[rs2 as usize] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Blt { rs1, rs2, offset } => {
                if (self.reg[rs1 as usize] as i64) < self.reg[rs2 as usize] as i64 {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bltu { rs1, rs2, offset } => {
                if self.reg[rs1 as usize] < self.reg[rs2 as usize] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bge { rs1, rs2, offset } => {
                if (self.reg[rs1 as usize] as i64) >= self.reg[rs2 as usize] as i64 {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
            Inst::Bgeu { rs1, rs2, offset } => {
                if self.reg[rs1 as usize] >= self.reg[rs2 as usize] {
                    self.pc = self.pc.wrapping_add(offset as u64).wrapping_sub(incr);
                }
            }
        }

        self.pc = self.pc.wrapping_add(incr);

        // make sure x0 is zero
        self.reg[0] = 0;
    }
}
