use std::fmt::Display;

use crate::emulator::{Reg, SP};

#[derive(Debug, PartialEq, Eq)]
pub enum Inst {
    // MISC.
    Fence,
    Ecall,
    Error(Box<str>), // not a risc v instruction but useful for control flow here
    Lui { rd: Reg, imm: u64 },

    // LOADS/STORES
    Ld { rd: Reg, rs1: Reg, offset: i32 },
    Lw { rd: Reg, rs1: Reg, offset: i32 },
    Lhu { rd: Reg, rs1: Reg, offset: i32 },
    Lbu { rd: Reg, rs1: Reg, offset: i32 },
    Sd { rs1: Reg, rs2: Reg, offset: i32 },
    Sw { rs1: Reg, rs2: Reg, offset: i32 },
    Sh { rs1: Reg, rs2: Reg, offset: i32 },
    Sb { rs1: Reg, rs2: Reg, offset: i32 },

    // MATH OPERATIONS
    Add { rd: Reg, rs1: Reg, rs2: Reg },
    Addw { rd: Reg, rs1: Reg, rs2: Reg },
    Addi { rd: Reg, rs1: Reg, imm: u64 },
    Addiw { rd: Reg, rs1: Reg, imm: u32 },
    Divu { rd: Reg, rs1: Reg, rs2: Reg },
    And { rd: Reg, rs1: Reg, rs2: Reg },
    Andi { rd: Reg, rs1: Reg, imm: u64 },
    Sub { rd: Reg, rs1: Reg, rs2: Reg },
    Subw { rd: Reg, rs1: Reg, rs2: Reg },
    Slli { rd: Reg, rs1: Reg, shamt: u64 },
    Slliw { rd: Reg, rs1: Reg, shamt: u32 },
    Srli { rd: Reg, rs1: Reg, shamt: u64 },
    Or { rd: Reg, rs1: Reg, rs2: Reg },
    Xor { rd: Reg, rs1: Reg, rs2: Reg },
    Xori { rd: Reg, rs1: Reg, imm: u64 },

    // JUMPING
    Auipc { rd: Reg, imm: u64 },
    Jal { rd: Reg, offset: u64 },
    Jalr { rd: Reg, rs1: Reg, offset: u64 },

    // BRANCHES
    Beq { rs1: Reg, rs2: Reg, offset: i32 },
    Bne { rs1: Reg, rs2: Reg, offset: i32 },
    Blt { rs1: Reg, rs2: Reg, offset: i32 },
    Bltu { rs1: Reg, rs2: Reg, offset: i32 },
    Bge { rs1: Reg, rs2: Reg, offset: i32 },
    Bgeu { rs1: Reg, rs2: Reg, offset: i32 },
    Mul { rd: Reg, rs1: Reg, rs2: Reg },
    Remu { rd: Reg, rs1: Reg, rs2: Reg },
    Srliw { rd: Reg, rs1: Reg, shamt: u32 },
}

impl Display for Inst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Inst::Fence => write!(f, "fence"),
            Inst::Ecall => write!(f, "ecall"),
            Inst::Error(ref e) => write!(f, "error: {e}"),
            Inst::Lui { rd, imm } => write!(f, "lui   {}, {:x}", rd, imm >> 12),
            Inst::Ld { rd, rs1, offset } => write!(f, "ld    {}, {}({})", rd, offset, rs1),
            Inst::Lw { rd, rs1, offset } => write!(f, "lw    {}, {}({})", rd, offset, rs1),
            Inst::Lhu { rd, rs1, offset } => write!(f, "lhu   {}, {}({})", rd, offset, rs1),
            Inst::Lbu { rd, rs1, offset } => write!(f, "lbu   {}, {}({})", rd, offset, rs1),
            Inst::Sd { rs1, rs2, offset } => write!(f, "sd    {}, {}({})", rs2, offset, rs1),
            Inst::Sw { rs1, rs2, offset } => write!(f, "sw    {}, {}({})", rs2, offset, rs1),
            Inst::Sh { rs1, rs2, offset } => write!(f, "sh    {}, {}({})", rs2, offset, rs1),
            Inst::Sb { rs1, rs2, offset } => write!(f, "sb    {}, {}({})", rs2, offset, rs1),
            Inst::Add { rd, rs1, rs2 } => write!(f, "add   {rd}, {rs1}, {rs2}"),
            Inst::Addw { rd, rs1, rs2 } => write!(f, "addw  {rd}, {rs1}, {rs2}"),
            Inst::Addi { rd, rs1, imm } => write!(f, "addi  {rd}, {rs1}, {}", imm as i64),
            Inst::Addiw { rd, rs1, imm } => write!(f, "addiw {rd}, {rs1}, {}", imm as i32),
            Inst::And { rd, rs1, rs2 } => write!(f, "and   {rd}, {rs1}, {rs2}"),
            Inst::Andi { rd, rs1, imm } => write!(f, "andi  {rd}, {rs1}, {}", imm as i64),
            Inst::Sub { rd, rs1, rs2 } => write!(f, "sub   {rd}, {rs1}, {rs2}"),
            Inst::Subw { rd, rs1, rs2 } => write!(f, "subw  {rd}, {rs1}, {rs2}"),
            Inst::Slli { rd, rs1, shamt } => write!(f, "slli  {rd}, {rs1}, {shamt}"),
            Inst::Slliw { rd, rs1, shamt } => write!(f, "slliw {rd}, {rs1}, {shamt}"),
            Inst::Srli { rd, rs1, shamt } => write!(f, "srli  {rd}, {rs1}, {shamt}"),
            Inst::Srliw { rd, rs1, shamt } => write!(f, "srliw {rd}, {rs1}, {shamt}"),
            Inst::Or { rd, rs1, rs2 } => write!(f, "or    {rd}, {rs1}, {rs2}"),
            Inst::Xor { rd, rs1, rs2 } => write!(f, "xor   {rd}, {rs1}, {rs2}"),
            Inst::Xori { rd, rs1, imm } => write!(f, "xori  {rd}, {rs1}, {imm}"),
            Inst::Auipc { rd, imm } => write!(f, "auipc {rd}, 0x{:x}", imm >> 12),
            Inst::Jal { rd, offset } => write!(f, "jal   {rd}, {offset:x}"),
            Inst::Jalr { rd, rs1, offset } => write!(f, "jalr  {rd}, {offset}({rs1})"),
            Inst::Beq { rs1, rs2, offset } => write!(f, "beq   {rs1}, {rs2}, {}", offset),
            Inst::Bne { rs1, rs2, offset } => write!(f, "bne   {rs1}, {rs2}, {}", offset),
            Inst::Blt { rs1, rs2, offset } => write!(f, "blt   {rs1}, {rs2}, {}", offset),
            Inst::Bltu { rs1, rs2, offset } => write!(f, "bltu  {rs1}, {rs2}, {}", offset),
            Inst::Bge { rs1, rs2, offset } => write!(f, "bge   {rs1}, {rs2}, {}", offset),
            Inst::Bgeu { rs1, rs2, offset } => write!(f, "bgeu  {rs1}, {rs2}, {}", offset),
            Inst::Divu { rd, rs1, rs2 } => write!(f, "divu  {rd}, {rs1}, {rs2}"),
            Inst::Mul { rd, rs1, rs2 } => write!(f, "mul   {rd}, {rs1}, {rs2}"),
            Inst::Remu { rd, rs1, rs2 } => write!(f, "remu  {rd}, {rs1}, {rs2}"),
        }
    }
}

impl Inst {
    // returns the instruction along with the number of bytes read
    pub fn decode(inst: u32) -> (Inst, u8) {
        match inst & 0b11 {
            0b00 | 0b01 | 0b10 => (Self::decode_compressed(inst as u16), 2),
            0b11 => (Self::decode_normal(inst), 4),
            _ => unreachable!(),
        }
    }

    fn decode_normal(inst: u32) -> Inst {
        let opcode = inst & 0b1111111;
        let rd = Reg(((inst >> 7) & 0b11111) as u8);
        let rs1 = Reg(((inst >> 15) & 0b11111) as u8);
        let rs2 = Reg(((inst >> 20) & 0b11111) as u8);
        let funct3 = (inst >> 12) & 0b111;
        let funct7 = (inst >> 25) & 0b1111111;

        match opcode {
            0b0000011 => {
                let offset = ((inst & 0xFFF00000) as i32) >> 20;

                match funct3 {
                    0b010 => Inst::Lw { rd, rs1, offset },
                    0b011 => Inst::Ld { rd, rs1, offset },
                    0b100 => Inst::Lbu { rd, rs1, offset },
                    0b101 => Inst::Lhu { rd, rs1, offset },
                    _ => {
                        Inst::Error(format!("unimplemented: a{opcode:b} funct3={funct3:b}").into())
                    }
                }
            }
            0b0001111 => Inst::Fence,
            0b0010011 => {
                let imm = ((inst & 0xFFF00000) as i32 as i64 >> 20) as u64;
                match funct3 {
                    0b000 => Inst::Addi { rd, rs1, imm },
                    0b001 => {
                        let shamt = ((inst >> 20) & 0b11111) as u64;
                        Inst::Slli { rd, rs1, shamt }
                    }
                    0b100 => Inst::Xori { rd, rs1, imm },
                    0b101 => {
                        let shamt = ((inst >> 20) & 0b11111) as u64;
                        Inst::Srli { rd, rs1, shamt }
                    }
                    0b111 => Inst::Andi { rd, rs1, imm },
                    _ => {
                        Inst::Error(format!("unimplemented: b{opcode:b} funct3={funct3:b}").into())
                    }
                }
            }

            // AUIPC - Add Upper Immediate to PC
            0b0010111 => {
                let imm = (inst & 0xFFFFF000) as i32 as i64 as u64;
                Inst::Auipc { rd, imm }
            }

            0b0011011 => match funct3 {
                0b000 => {
                    let imm = ((inst & 0b11111111111100000000000000000000) as i32 >> 20) as u32;
                    Inst::Addiw { rd, rs1, imm }
                }
                0b001 => {
                    assert_eq!(funct7, 0);
                    let shamt = ((inst >> 20) & 0b111111) as u32;
                    Inst::Slliw { rd, rs1, shamt }
                }
                0b101 => {
                    assert_eq!(funct7, 0); // TODO: handle SRAIW
                    let shamt = ((inst >> 20) & 0b111111) as u32;
                    Inst::Srliw { rd, rs1, shamt }
                }
                _ => Inst::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
            },

            // STORE
            0b0100011 => {
                let offset = ((inst & 0b11111110000000000000000000000000) as i32) >> 20 // imm[11:5]
                           | (inst & 0b111110000000) as i32 >> 7; // imm[4:0]

                match funct3 {
                    0b011 => Inst::Sd { rs1, rs2, offset },
                    0b010 => Inst::Sw { rs1, rs2, offset },
                    0b001 => Inst::Sh { rs1, rs2, offset },
                    0b000 => Inst::Sb { rs1, rs2, offset },
                    _ => Inst::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
                }
            }

            0b0110011 => match funct3 {
                0b000 => match funct7 {
                    0b0000000 => Inst::Add { rd, rs1, rs2 },
                    0b0100000 => Inst::Sub { rd, rs1, rs2 },
                    0b0000001 => Inst::Mul { rd, rs1, rs2 },
                    _ => panic!("Invalid instruction"),
                },
                0b101 => match funct7 {
                    0b0000001 => Inst::Divu { rd, rs1, rs2 },
                    _ => panic!("Boojookieland"),
                },
                0b111 => match funct7 {
                    0b0000001 => Inst::Remu { rd, rs1, rs2 },
                    _ => panic!("Zoinks!"),
                },
                0b110 => match funct7 {
                    0b0000000 => Inst::Or { rd, rs1, rs2 },
                    _ => panic!("Orange you glad you're not this instruction."),
                },
                _ => Inst::Error(format!("Invalid for thing").into()),
            },
            0b0110111 => {
                let imm = (inst & 0xFFFFF000) as i32 as u64;

                Inst::Lui { rd, imm }
            }

            0b0111011 => match funct7 {
                0b0000000 => Inst::Addw { rd, rs1, rs2 },
                0b0100000 => Inst::Subw { rd, rs1, rs2 },
                _ => panic!("opcode={:07b} unimplemented", opcode),
            },

            // Branches
            0b1100011 => {
                let offset = ((inst & 0b1111110000000000000000000000000) >> 20) as i32  // imm[10:5]
                           | ((inst & 0b10000000000000000000000000000000) as i32 >> 19) // imm[12]
                           | ((inst & 0b10000000) << 4) as i32 // imm[11]
                           | ((inst & 0b111100000000) >> 7) as i32; // imm[4:1]

                match funct3 {
                    0b000 => Inst::Beq { rs1, rs2, offset },
                    0b001 => Inst::Bne { rs1, rs2, offset },
                    0b100 => Inst::Blt { rs1, rs2, offset },
                    0b101 => Inst::Bge { rs1, rs2, offset },
                    0b110 => Inst::Bltu { rs1, rs2, offset },
                    0b111 => Inst::Bgeu { rs1, rs2, offset },
                    _ => Inst::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
                }
            }

            0b1101111 => {
                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let offset = (((inst & 0x80000000) as i32 as i64 >> 11) as u64) // imm[20]
                           | (inst & 0xff000) as u64 // imm[19:12]
                           | ((inst >> 9) & 0x800) as u64 // imm[11]
                           | ((inst >> 20) & 0x7fe) as u64; // imm[10:1]

                Inst::Jal { rd, offset }
            }

            0b1110011 => Inst::Ecall,

            _ => Inst::Error(format!("unimplemented: {opcode:07b}").into()),
        }
    }

    fn decode_compressed(inst: u16) -> Inst {
        let quadrant = inst & 0b11;
        let funct3 = (inst >> 13) & 0b111;

        match quadrant {
            0b00 => {
                match funct3 {
                    0b000 => {
                        // C.ADDI4SPN

                        // nzuimm
                        let imm = (inst & 0b100000) >> 2 // imm[3]
                                | (inst & 0b1000000) >> 4 // imm[2]
                                | (inst & 0b11110000000) >> 1 // imm[9:6]
                                | (inst & 0b1100000000000) >> 7; // imm[5:4]
                        let rd = Reg((((inst >> 2) & 0b111) + 8) as u8);

                        Inst::Addi {
                            rd,
                            rs1: SP,
                            imm: imm as u64,
                        }
                    }
                    0b010 => {
                        // C.LW
                        let rd = Reg((((inst >> 2) & 0b111) + 8) as u8);
                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);
                        let offset = (inst & 0b100000) << 1 // imm[6]
                                   | (inst & 0b1000000) >> 4 // imm[2]
                                   | (inst & 0b1110000000000) >> 7; // imm[5:3]

                        Inst::Lw {
                            rd,
                            rs1,
                            offset: offset as i32,
                        }
                    }
                    0b011 => {
                        // C.LD
                        let rd = Reg((((inst >> 2) & 0b111) + 8) as u8);
                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);
                        let offset = (inst & 0b1100000) << 1 // imm[7:6]
                                   | (inst & 0b1110000000000) >> 7; // imm[5:3]

                        Inst::Ld {
                            rd,
                            rs1,
                            offset: offset as i32,
                        }
                    }
                    0b110 => {
                        // C.SW
                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);
                        let rs2 = Reg((((inst >> 2) & 0b111) + 8) as u8);
                        let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                                | (inst & 0b100000) << 1 // imm[6]
                                | (inst & 0b1000000) >> 4; // imm[2]

                        Inst::Sw {
                            rs1,
                            rs2,
                            offset: imm as i32,
                        }
                    }
                    0b111 => {
                        // C.SD
                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);
                        let rs2 = Reg((((inst >> 2) & 0b111) + 8) as u8);
                        let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                                | (inst & 0b1100000) << 1; // imm[7:6]

                        Inst::Sd {
                            rs1,
                            rs2,
                            offset: imm as i32,
                        }
                    }
                    _ => Inst::Error(
                        format!("unimplemented: quadrant={quadrant:02b} {funct3:03b} {inst:x}")
                            .into(),
                    ),
                }
            }
            0b01 => {
                match funct3 {
                    0b000 => {
                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        Inst::Addi {
                            rd,
                            rs1: rd,
                            imm: imm as u64,
                        }
                    }
                    0b001 => {
                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        Inst::Addiw {
                            rd,
                            rs1: rd,
                            imm: imm as u32,
                        }
                    }
                    0b010 => {
                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        Inst::Addi {
                            rd,
                            rs1: Reg(0),
                            imm: imm as u64,
                        }
                    }
                    0b011 => {
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        if rd == Reg(2) {
                            // C.ADDI16SP
                            let imm = (((inst & 0b1000000000000) << 3) as i16 >> 6) as u64 // imm[9]
                                    | ((inst & 0b100) << 3) as u64 // imm[5]
                                    | ((inst & 0b11000) << 4) as u64 // imm[8:7]
                                    | ((inst & 0b100000) << 1) as u64 // imm[6]
                                    | ((inst & 0b1000000) >> 2) as u64; // imm[4]

                            Inst::Addi {
                                rd: SP,
                                rs1: SP,
                                imm,
                            }
                        } else {
                            // C.LUI
                            let imm = ((((inst & 0b1000000000000) << 3) as i16 as i32) << 2)  // imm[17]
                                    | ((inst as u32 & 0b1111100) << 10) as i32; // imm[16:12]

                            Inst::Lui {
                                rd,
                                imm: imm as u64,
                            }
                        }
                    }
                    0b100 => {
                        let funct2 = (inst >> 10) & 0b11;
                        let rd = Reg((((inst >> 7) & 0b111) + 8) as u8);

                        match funct2 {
                            // C.SRLI
                            0b00 => {
                                let shamt = (inst & 0b1000000000000) >> 7 // imm[5]
                                          | (inst & 0b1111100) >> 2; // imm[4:0]

                                if shamt == 0 {
                                    panic!("Immediate must be nonzero");
                                }

                                Inst::Srli {
                                    rd,
                                    rs1: rd,
                                    shamt: shamt as u64,
                                }
                            }

                            // C.ANDI
                            0b10 => {
                                let imm = ((inst & 0b1000000000000) << 3) as i16 >> 10 // imm[5]
                                        | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                                Inst::Andi {
                                    rd,
                                    rs1: rd,
                                    imm: imm as u64,
                                }
                            }

                            0b11 => {
                                let funct3 = (((inst >> 12) & 0b1) << 2) | (inst >> 5) & 0b11;
                                let rs2 = Reg((((inst >> 2) & 0b111) + 8) as u8);

                                match funct3 {
                                    0b000 => Inst::Sub { rd, rs1: rd, rs2 },
                                    0b001 => Inst::Xor { rd, rs1: rd, rs2 },
                                    0b010 => Inst::Or { rd, rs1: rd, rs2 },
                                    0b011 => Inst::And { rd, rs1: rd, rs2 },
                                    0b100 => Inst::Subw { rd, rs1: rd, rs2 },
                                    0b101 => Inst::Addw { rd, rs1: rd, rs2 },

                                    _ => {
                                        unreachable!();
                                    }
                                }
                            }
                            _ => Inst::Error(
                                format!("unimplemented instruction: {inst:0b} {funct2:0b}").into(),
                            ),
                        }
                    }
                    0b101 => {
                        let imm = (inst & 0b100) << 3 // imm[5]
                                | (inst & 0b111000) >> 2 // imm[3:1]
                                | (inst & 0b1000000) << 1 // imm[7]
                                | (inst & 0b10000000) >> 1 // imm[6]
                                | (inst & 0b100000000) << 2 // imm[10]
                                | (inst & 0b11000000000) >> 1 // imm[9:8]
                                | (inst & 0b100000000000) >> 7 // imm[4]
                                | (((inst & 0b1000000000000) << 3) as i16 >> 4) as u16; // imm[11]

                        Inst::Jal {
                            rd: Reg(0),
                            offset: imm as i16 as u64,
                        }
                    }
                    0b110 => {
                        // C.BEQZ
                        let offset = ((inst & 0b110000000000) >> 7) as i32 // imm[4:3]
                                   | (((inst & 0b1000000000000) >> 5) as i8 as i32) << 1 // imm[8]
                                   | ((inst & 0b100) << 3) as i32 // imm[5]
                                   | ((inst & 0b11000) >> 2) as i32 // imm[2:1]
                                   | ((inst & 0b1100000) << 1) as i32; // imm[7:6]

                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);

                        Inst::Beq {
                            rs1,
                            rs2: Reg(0),
                            offset,
                        }
                    }
                    0b111 => {
                        // C.BNEZ
                        let offset = ((inst & 0b110000000000) >> 7) as i32 // imm[4:3]
                                   | (((inst & 0b1000000000000) >> 5) as i8 as i32) << 1 // imm[8]
                                   | ((inst & 0b100) << 3) as i32 // imm[5]
                                   | ((inst & 0b11000) >> 2) as i32 // imm[2:1]
                                   | ((inst & 0b1100000) << 1) as i32; // imm[7:6]

                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);

                        Inst::Bne {
                            rs1,
                            rs2: Reg(0),
                            offset,
                        }
                    }
                    _ => Inst::Error(
                        format!("unimplemented: quadrant={quadrant:02b} {funct3:03b}").into(),
                    ),
                }
            }
            0b10 => {
                match funct3 {
                    0b000 => {
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);
                        let shamt = (inst & 0b1000000000000) >> 7 // imm[5]
                                  | (inst & 0b1111100) >> 2; // imm[4:0]

                        Inst::Slli {
                            rd,
                            rs1: rd,
                            shamt: shamt as u64,
                        }
                    }
                    0b010 => {
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);
                        let imm = (inst & 0b1100) << 4 // imm[7:6]
                                | (inst & 0b1110000) >> 2 // imm[4:2]
                                | (inst & 0b1000000000000) >> 7; // imm[5]

                        // C.LWSP
                        if rd != Reg(0) {
                            // C.LDSP
                            Inst::Lw {
                                rd,
                                rs1: SP,
                                offset: imm as i32,
                            }
                        } else {
                            panic!("Invalid instruction");
                        }
                    }
                    0b011 => {
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);
                        let imm = (inst & 0b1000000000000) >> 7 // imm[5]
                                | (inst & 0b11100) << 4 // imm[8:6]
                                | (inst & 0b1100000) >> 2; // imm[4:3]

                        if rd != Reg(0) {
                            // C.LDSP
                            Inst::Ld {
                                rd,
                                rs1: SP,
                                offset: imm as i32,
                            }
                        } else {
                            panic!("C.FLWSP not implemented");
                        }
                    }
                    0b100 => {
                        let imm = (inst >> 12) & 0b1;
                        let rs1 = Reg(((inst >> 7) & 0b11111) as u8);
                        let rs2 = Reg(((inst >> 2) & 0b11111) as u8);

                        // C.JR - ret
                        if imm == 0 && rs1 != Reg(0) && rs2 == Reg(0) {
                            Inst::Jalr {
                                rd: Reg(0),
                                rs1,
                                offset: 0,
                            }
                        }
                        // C.MV - Move
                        else if imm == 0 && rs1 != Reg(0) && rs2 != Reg(0) {
                            Inst::Add {
                                rd: rs1,
                                rs1: Reg(0),
                                rs2,
                            }
                        }
                        // C.ADD - Add
                        else if imm == 1 && rs1 != Reg(0) && rs2 != Reg(0) {
                            Inst::Add { rd: rs1, rs1, rs2 }
                        } else {
                            Inst::Error(
                                format!("compressed instruction `{inst:016b}` not implemented.")
                                    .into(),
                            )
                        }
                    }
                    0b111 => {
                        // C.SDSP - not C.SWSP since we are emulating RV64C
                        let offset =
                            (((inst >> 7) & 0b111000) | ((inst >> 1) & 0b111000000)) as i32;
                        let rs2 = Reg(((inst >> 2) & 0b11111) as u8);

                        Inst::Sd {
                            rs1: SP,
                            rs2,
                            offset,
                        }
                    }
                    _ => Inst::Error(format!("quadrant={quadrant:02b} funct3={funct3:03b}").into()),
                }
            }
            0b11 => Inst::Error("Quadrant 11 should not exist".into()),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::*;

    #[test_log::test]
    fn cload_decoding() {
        let (inst, _) = Inst::decode(0x0000639c);
        assert_eq!(
            inst,
            Inst::Ld {
                rd: A5,
                rs1: A5,
                offset: 0
            }
        );

        // C.LDSP
        let (inst, _) = Inst::decode(0x000046ca);
        assert_eq!(
            inst,
            Inst::Lw {
                rd: A3,
                rs1: SP,
                offset: 144
            }
        );
    }

    #[test_log::test]
    fn xori_decoding() {
        let (inst, _) = Inst::decode(0xfff64613);
        assert_eq!(
            inst,
            Inst::Xori {
                rd: A2,
                rs1: A2,
                imm: -1i64 as u64
            },
        );
    }

    #[test_log::test]
    fn srliw_decoding() {
        let (inst, _) = Inst::decode(0x0087d49b);
        assert_eq!(
            inst,
            Inst::Srliw {
                rd: S1,
                rs1: A5,
                shamt: 8,
            }
        );
    }

    #[test_log::test]
    fn add_sub_decoding() {
        let (inst, _) = Inst::decode(0x00c58533);
        assert_eq!(
            inst,
            Inst::Add {
                rd: A0,
                rs1: A1,
                rs2: A2
            }
        );

        let (inst, _) = Inst::decode(0x40c58533);
        assert_eq!(
            inst,
            Inst::Sub {
                rd: A0,
                rs1: A1,
                rs2: A2
            }
        );

        let (inst, _) = Inst::decode(0x02c5d533);
        assert_eq!(
            inst,
            Inst::Divu {
                rd: A0,
                rs1: A1,
                rs2: A2
            }
        );

        let (inst, _) = Inst::decode(0x02c58533);
        assert_eq!(
            inst,
            Inst::Mul {
                rd: A0,
                rs1: A1,
                rs2: A2
            }
        );

        let (inst, _) = Inst::decode(0x02c5f533);
        assert_eq!(
            inst,
            Inst::Remu {
                rd: A0,
                rs1: A1,
                rs2: A2
            }
        );
    }
}
