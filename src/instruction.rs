use std::fmt::Display;

use crate::register::{FReg, Reg, RA, SP};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Inst {
    // MISC.
    Fence,
    Ecall,
    Ebreak,
    Error(u32),
    Lui { rd: Reg, imm: i32 },

    // LOADS/STORES
    Ld { rd: Reg, rs1: Reg, offset: i32 },
    Lw { rd: Reg, rs1: Reg, offset: i32 },
    Lwu { rd: Reg, rs1: Reg, offset: i32 },
    Lhu { rd: Reg, rs1: Reg, offset: i32 },
    Lb { rd: Reg, rs1: Reg, offset: i32 },
    Lbu { rd: Reg, rs1: Reg, offset: i32 },
    Sd { rs1: Reg, rs2: Reg, offset: i32 },
    Sw { rs1: Reg, rs2: Reg, offset: i32 },
    Sh { rs1: Reg, rs2: Reg, offset: i32 },
    Sb { rs1: Reg, rs2: Reg, offset: i32 },

    // MATH OPERATIONS
    Add { rd: Reg, rs1: Reg, rs2: Reg },
    Addw { rd: Reg, rs1: Reg, rs2: Reg },
    Addi { rd: Reg, rs1: Reg, imm: i32 },
    Addiw { rd: Reg, rs1: Reg, imm: u32 },
    Div { rd: Reg, rs1: Reg, rs2: Reg },
    Divw { rd: Reg, rs1: Reg, rs2: Reg },
    Divu { rd: Reg, rs1: Reg, rs2: Reg },
    Divuw { rd: Reg, rs1: Reg, rs2: Reg },
    And { rd: Reg, rs1: Reg, rs2: Reg },
    Andi { rd: Reg, rs1: Reg, imm: i32 },
    Sub { rd: Reg, rs1: Reg, rs2: Reg },
    Subw { rd: Reg, rs1: Reg, rs2: Reg },
    Sll { rd: Reg, rs1: Reg, rs2: Reg },
    Sllw { rd: Reg, rs1: Reg, rs2: Reg },
    Slli { rd: Reg, rs1: Reg, shamt: u32 },
    Slliw { rd: Reg, rs1: Reg, shamt: u32 },
    Srl { rd: Reg, rs1: Reg, rs2: Reg },
    Srlw { rd: Reg, rs1: Reg, rs2: Reg },
    Srli { rd: Reg, rs1: Reg, shamt: u32 },
    Srliw { rd: Reg, rs1: Reg, shamt: u32 },
    Sra { rd: Reg, rs1: Reg, rs2: Reg },
    Sraw { rd: Reg, rs1: Reg, rs2: Reg },
    Srai { rd: Reg, rs1: Reg, shamt: u32 },
    Sraiw { rd: Reg, rs1: Reg, shamt: u32 },
    Or { rd: Reg, rs1: Reg, rs2: Reg },
    Ori { rd: Reg, rs1: Reg, imm: i32 },
    Xor { rd: Reg, rs1: Reg, rs2: Reg },
    Xori { rd: Reg, rs1: Reg, imm: i32 },

    // JUMPING
    Auipc { rd: Reg, imm: i32 },
    Jal { rd: Reg, offset: i32 },
    Jalr { rd: Reg, rs1: Reg, offset: i32 },

    // BRANCHES
    Beq { rs1: Reg, rs2: Reg, offset: i32 },
    Bne { rs1: Reg, rs2: Reg, offset: i32 },
    Blt { rs1: Reg, rs2: Reg, offset: i32 },
    Bltu { rs1: Reg, rs2: Reg, offset: i32 },
    Bge { rs1: Reg, rs2: Reg, offset: i32 },
    Bgeu { rs1: Reg, rs2: Reg, offset: i32 },
    Mul { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhu { rd: Reg, rs1: Reg, rs2: Reg },
    Remw { rd: Reg, rs1: Reg, rs2: Reg },
    Remu { rd: Reg, rs1: Reg, rs2: Reg },
    Remuw { rd: Reg, rs1: Reg, rs2: Reg },
    Slt { rd: Reg, rs1: Reg, rs2: Reg },
    Sltu { rd: Reg, rs1: Reg, rs2: Reg },
    Slti { rd: Reg, rs1: Reg, imm: i32 },
    Sltiu { rd: Reg, rs1: Reg, imm: u32 },

    // ATOMICS
    Amoswapw { rd: Reg, rs1: Reg, rs2: Reg },
    Amoswapd { rd: Reg, rs1: Reg, rs2: Reg },
    Amoaddw { rd: Reg, rs1: Reg, rs2: Reg },
    Amoaddd { rd: Reg, rs1: Reg, rs2: Reg },
    Amoorw { rd: Reg, rs1: Reg, rs2: Reg },
    Amomaxuw { rd: Reg, rs1: Reg, rs2: Reg },
    Amomaxud { rd: Reg, rs1: Reg, rs2: Reg },
    Lrw { rd: Reg, rs1: Reg },
    Lrd { rd: Reg, rs1: Reg },
    Scw { rd: Reg, rs1: Reg, rs2: Reg },
    Scd { rd: Reg, rs1: Reg, rs2: Reg },

    // FLOATING POINT
    Fsd { rs1: Reg, rs2: FReg, offset: i32 },
    Fsw { rs1: Reg, rs2: FReg, offset: i32 },
    Fld { rd: FReg, rs1: Reg, offset: i32 },
    Flw { rd: FReg, rs1: Reg, offset: i32 },
    Fcvtdlu { rd: Reg, rs1: FReg, rm: u8 },
    Fcvtds { rd: Reg, rs1: FReg, rm: u8 },
    Fled { rd: Reg, rs1: FReg, rs2: FReg },
    Fdivd { rd: FReg, rs1: FReg, rs2: FReg },
}

impl Inst {
    pub fn fmt(&self, pc: u64) -> String {
        match *self {
            Inst::Fence => format!("fence"),
            Inst::Ecall => format!("ecall"),
            Inst::Ebreak => format!("break"),
            Inst::Error(ref e) => format!("error: {e:08x}"),
            Inst::Lui { rd, imm } => format!("lui   {}, {:x}", rd, imm >> 12),
            Inst::Ld { rd, rs1, offset } => format!("ld    {}, {}({})", rd, offset, rs1),
            Inst::Lw { rd, rs1, offset } => format!("lw    {}, {}({})", rd, offset, rs1),
            Inst::Lwu { rd, rs1, offset } => format!("lwu    {}, {}({})", rd, offset, rs1),
            Inst::Lhu { rd, rs1, offset } => format!("lhu   {}, {}({})", rd, offset, rs1),
            Inst::Lb { rd, rs1, offset } => format!("lbu   {}, {}({})", rd, offset, rs1),
            Inst::Lbu { rd, rs1, offset } => format!("lbu   {}, {}({})", rd, offset, rs1),
            Inst::Sd { rs1, rs2, offset } => format!("sd    {}, {}({})", rs2, offset, rs1),
            Inst::Sw { rs1, rs2, offset } => format!("sw    {}, {}({})", rs2, offset, rs1),
            Inst::Sh { rs1, rs2, offset } => format!("sh    {}, {}({})", rs2, offset, rs1),
            Inst::Sb { rs1, rs2, offset } => format!("sb    {}, {}({})", rs2, offset, rs1),
            Inst::Add { rd, rs1, rs2 } => format!("add   {rd}, {rs1}, {rs2}"),
            Inst::Addw { rd, rs1, rs2 } => format!("addw  {rd}, {rs1}, {rs2}"),
            Inst::Addi { rd, rs1, imm } => format!("addi  {rd}, {rs1}, {}", imm as i64),
            Inst::Addiw { rd, rs1, imm } => format!("addiw {rd}, {rs1}, {}", imm as i32),
            Inst::And { rd, rs1, rs2 } => format!("and   {rd}, {rs1}, {rs2}"),
            Inst::Andi { rd, rs1, imm } => format!("andi  {rd}, {rs1}, {}", imm as i64),
            Inst::Sub { rd, rs1, rs2 } => format!("sub   {rd}, {rs1}, {rs2}"),
            Inst::Subw { rd, rs1, rs2 } => format!("subw  {rd}, {rs1}, {rs2}"),
            Inst::Sll { rd, rs1, rs2 } => format!("sll  {rd}, {rs1}, {rs2}"),
            Inst::Sllw { rd, rs1, rs2 } => format!("sllw  {rd}, {rs1}, {rs2}"),
            Inst::Slli { rd, rs1, shamt } => format!("slli  {rd}, {rs1}, {shamt}"),
            Inst::Slliw { rd, rs1, shamt } => format!("slliw {rd}, {rs1}, {shamt}"),
            Inst::Srl { rd, rs1, rs2 } => format!("srl  {rd}, {rs1}, {rs2}"),
            Inst::Srlw { rd, rs1, rs2 } => format!("srl  {rd}, {rs1}, {rs2}"),
            Inst::Srli { rd, rs1, shamt } => format!("srli  {rd}, {rs1}, {shamt}"),
            Inst::Srliw { rd, rs1, shamt } => format!("srliw {rd}, {rs1}, {shamt}"),
            Inst::Sra { rd, rs1, rs2 } => format!("sra  {rd}, {rs1}, {rs2}"),
            Inst::Sraw { rd, rs1, rs2 } => format!("sraw {rd}, {rs1}, {rs2}"),
            Inst::Srai { rd, rs1, shamt } => format!("srai  {rd}, {rs1}, {shamt}"),
            Inst::Sraiw { rd, rs1, shamt } => format!("sraiw {rd}, {rs1}, {shamt}"),
            Inst::Or { rd, rs1, rs2 } => format!("or    {rd}, {rs1}, {rs2}"),
            Inst::Ori { rd, rs1, imm } => format!("ori   {rd}, {rs1}, {imm}"),
            Inst::Xor { rd, rs1, rs2 } => format!("xor   {rd}, {rs1}, {rs2}"),
            Inst::Xori { rd, rs1, imm } => format!("xori  {rd}, {rs1}, {imm}"),
            Inst::Auipc { rd, imm } => format!("auipc {rd}, 0x{:x}", imm as u64 >> 12),
            Inst::Jal { rd, offset } => format!("jal   {rd}, {:x}", pc.wrapping_add(offset as u64)),
            Inst::Jalr { rd, rs1, offset } => format!("jalr  {rd}, {offset}({rs1})"),
            Inst::Beq { rs1, rs2, offset } => {
                format!("beq   {rs1}, {rs2}, {:x}", pc.wrapping_add(offset as u64))
            }
            Inst::Bne { rs1, rs2, offset } => {
                format!("bne   {rs1}, {rs2}, {:x}", pc.wrapping_add(offset as u64))
            }
            Inst::Blt { rs1, rs2, offset } => {
                format!("blt   {rs1}, {rs2}, {:x}", pc.wrapping_add(offset as u64))
            }
            Inst::Bltu { rs1, rs2, offset } => {
                format!("bltu  {rs1}, {rs2}, {:x}", pc.wrapping_add(offset as u64))
            }
            Inst::Bge { rs1, rs2, offset } => {
                format!("bge   {rs1}, {rs2}, {:x}", pc.wrapping_add(offset as u64))
            }
            Inst::Bgeu { rs1, rs2, offset } => {
                format!("bgeu  {rs1}, {rs2}, {:x}", pc.wrapping_add(offset as u64))
            }
            Inst::Div { rd, rs1, rs2 } => format!("div   {rd}, {rs1}, {rs2}"),
            Inst::Divw { rd, rs1, rs2 } => format!("divw  {rd}, {rs1}, {rs2}"),
            Inst::Divu { rd, rs1, rs2 } => format!("divu  {rd}, {rs1}, {rs2}"),
            Inst::Divuw { rd, rs1, rs2 } => format!("divuw {rd}, {rs1}, {rs2}"),
            Inst::Mul { rd, rs1, rs2 } => format!("mul   {rd}, {rs1}, {rs2}"),
            Inst::Mulhu { rd, rs1, rs2 } => format!("mul   {rd}, {rs1}, {rs2}"),
            Inst::Remw { rd, rs1, rs2 } => format!("remw  {rd}, {rs1}, {rs2}"),
            Inst::Remu { rd, rs1, rs2 } => format!("remu  {rd}, {rs1}, {rs2}"),
            Inst::Remuw { rd, rs1, rs2 } => format!("remuw  {rd}, {rs1}, {rs2}"),
            Inst::Amoswapw { rd, rs1, rs2 } => format!("amoswap.w {rd}, {rs1}, {rs2}"),
            Inst::Amoswapd { rd, rs1, rs2 } => format!("amoswap.d {rd}, {rs1}, {rs2}"),
            Inst::Amoaddw { rd, rs1, rs2 } => format!("amoadd.w {rd}, {rs1}, {rs2}"),
            Inst::Amoaddd { rd, rs1, rs2 } => format!("amoadd.d {rd}, {rs1}, {rs2}"),
            Inst::Amoorw { rd, rs1, rs2 } => format!("amoor.w {rd}, {rs1}, {rs2}"),
            Inst::Amomaxuw { rd, rs1, rs2 } => format!("amomaxu.w {rd}, {rs1}, {rs2}"),
            Inst::Amomaxud { rd, rs1, rs2 } => format!("amomaxu.d {rd}, {rs1}, {rs2}"),
            Inst::Slt { rd, rs1, rs2 } => format!("slt   {rd}, {rs1}, {rs2}"),
            Inst::Sltu { rd, rs1, rs2 } => format!("sltu  {rd}, {rs1}, {rs2}"),
            Inst::Slti { rd, rs1, imm } => format!("slti  {rd}, {rs1}, {imm}"),
            Inst::Sltiu { rd, rs1, imm } => format!("sltiu {rd}, {rs1}, {imm}"),
            Inst::Lrw { rd, rs1 } => format!("lr.w  {rd}, ({rs1})"),
            Inst::Lrd { rd, rs1 } => format!("lr.d  {rd}, ({rs1})"),
            Inst::Scw { rd, rs1, rs2 } => format!("sc.w  {rd}, {rs2},({rs1})"),
            Inst::Scd { rd, rs1, rs2 } => format!("sc.d  {rd}, {rs2},({rs1})"),
            Inst::Fsd { rs1, rs2, offset } => format!("fsd   {rs2}, {offset}({rs1})"),
            Inst::Fsw { rs1, rs2, offset } => format!("fsw   {rs2}, {offset}({rs1})"),
            Inst::Fld { rs1, rd, offset } => format!("fld   {rd}, {offset}({rs1})"),
            Inst::Flw { rs1, rd, offset } => format!("flw   {rd}, {offset}({rs1})"),
            Inst::Fcvtdlu { rs1, rd, rm } => format!("fcvt.d.lu {rd}, {rs1} rm={rm:03b}"),
            Inst::Fcvtds { rs1, rd, rm } => format!("fcvt.d.s {rd}, {rs1} rm={rm:03b}"),
            Inst::Fled { rd, rs1, rs2 } => format!("fle.d  {rd}, {rs1} {rs2}"),
            Inst::Fdivd { rd, rs1, rs2 } => format!("fdiv.d {rd}, {rs1} {rs2}"),
        }
    }

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
        let funct5 = (inst >> 27) & 0b11111;
        let funct6 = (inst >> 26) & 0b111111;
        let funct7 = (inst >> 25) & 0b1111111;

        match opcode {
            0b0000011 => {
                let offset = ((inst & 0xFFF00000) as i32) >> 20;

                match funct3 {
                    0b000 => Inst::Lb { rd, rs1, offset },
                    0b010 => Inst::Lw { rd, rs1, offset },
                    0b011 => Inst::Ld { rd, rs1, offset },
                    0b100 => Inst::Lbu { rd, rs1, offset },
                    0b101 => Inst::Lhu { rd, rs1, offset },
                    0b110 => Inst::Lwu { rd, rs1, offset },
                    _ => Inst::Error(inst),
                }
            }
            0b0000111 => {
                let offset = (inst & 0xFFF00000) as i32 >> 20;
                match funct3 {
                    0b010 => Inst::Flw {
                        rd: FReg(rd.0),
                        rs1,
                        offset,
                    },
                    0b011 => Inst::Fld {
                        rd: FReg(rd.0),
                        rs1,
                        offset,
                    },
                    _ => Inst::Error(inst),
                }
            }
            0b0001111 => Inst::Fence,
            0b0010011 => {
                let imm = (inst & 0xFFF00000) as i32 >> 20;
                match funct3 {
                    0b000 => Inst::Addi { rd, rs1, imm },
                    0b001 => {
                        let shamt = (inst >> 20) & 0b111111;
                        Inst::Slli { rd, rs1, shamt }
                    }
                    0b010 => Inst::Slti { rd, rs1, imm },
                    0b011 => Inst::Sltiu {
                        rd,
                        rs1,
                        imm: imm as u32,
                    },
                    0b100 => Inst::Xori { rd, rs1, imm },
                    0b101 => match funct6 {
                        0b000000 => {
                            let shamt = (inst >> 20) & 0b111111;
                            Inst::Srli { rd, rs1, shamt }
                        }
                        0b010000 => {
                            let shamt = (inst >> 20) & 0b111111;
                            Inst::Srai { rd, rs1, shamt }
                        }
                        _ => Inst::Error(inst),
                    },
                    0b110 => Inst::Ori { rd, rs1, imm },
                    0b111 => Inst::Andi { rd, rs1, imm },
                    _ => Inst::Error(inst),
                }
            }

            // AUIPC - Add Upper Immediate to PC
            0b0010111 => {
                let imm = (inst & 0xFFFFF000) as i32;
                Inst::Auipc { rd, imm }
            }

            0b0011011 => match funct3 {
                0b000 => {
                    let imm = ((inst & 0b11111111111100000000000000000000) as i32 >> 20) as u32;
                    Inst::Addiw { rd, rs1, imm }
                }
                0b001 => match funct7 {
                    0b0000000 => {
                        let shamt = ((inst >> 20) & 0b11111) as u32;
                        Inst::Slliw { rd, rs1, shamt }
                    }
                    _ => Inst::Error(inst),
                },
                0b101 => {
                    let shamt = ((inst >> 20) & 0b11111) as u32;
                    match funct7 {
                        0b0000000 => Inst::Srliw { rd, rs1, shamt },
                        0b0100000 => Inst::Sraiw { rd, rs1, shamt },
                        _ => Inst::Error(inst),
                    }
                }
                _ => Inst::Error(inst),
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
                    _ => Inst::Error(inst),
                }
            }

            0b0100111 => {
                let offset = ((inst & 0b11111110000000000000000000000000) as i32) >> 20 // imm[11:5]
                           | (inst & 0b111110000000) as i32 >> 7; // imm[4:0]

                match funct3 {
                    0b010 => Inst::Fsw {
                        rs2: FReg(rs2.0),
                        rs1,
                        offset,
                    },

                    0b011 => Inst::Fsd {
                        rs2: FReg(rs2.0),
                        rs1,
                        offset,
                    },
                    _ => Inst::Error(inst),
                }
            }

            0b0110011 => match funct3 {
                0b000 => match funct7 {
                    0b0000000 => Inst::Add { rd, rs1, rs2 },
                    0b0100000 => Inst::Sub { rd, rs1, rs2 },
                    0b0000001 => Inst::Mul { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b001 => match funct7 {
                    0b0000000 => Inst::Sll { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b010 => match funct7 {
                    0b0000000 => Inst::Slt { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b011 => match funct7 {
                    0b0000000 => Inst::Sltu { rd, rs1, rs2 },
                    0b0000001 => Inst::Mulhu { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b100 => match funct7 {
                    0b0000000 => Inst::Xor { rd, rs1, rs2 },
                    0b0000001 => Inst::Div { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b101 => match funct7 {
                    0b0000000 => Inst::Srl { rd, rs1, rs2 },
                    0b0000001 => Inst::Divu { rd, rs1, rs2 },
                    0b0100000 => Inst::Sra { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },

                0b111 => match funct7 {
                    0b0000000 => Inst::And { rd, rs1, rs2 },
                    0b0000001 => Inst::Remu { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b110 => match funct7 {
                    0b0000000 => Inst::Or { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                _ => Inst::Error(inst),
            },
            0b0110111 => {
                let imm = (inst & 0xFFFFF000) as i32;

                Inst::Lui { rd, imm }
            }

            0b0111011 => match funct3 {
                0b000 => match funct7 {
                    0b0000000 => Inst::Addw { rd, rs1, rs2 },
                    0b0100000 => Inst::Subw { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b001 => match funct7 {
                    0b0000000 => Inst::Sllw { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b100 => match funct7 {
                    0b0000001 => Inst::Divw { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b101 => match funct7 {
                    0b0000000 => Inst::Srlw { rd, rs1, rs2 },
                    0b0000001 => Inst::Divuw { rd, rs1, rs2 },
                    0b0100000 => Inst::Sraw { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b110 => match funct7 {
                    0b0000001 => Inst::Remw { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                0b111 => match funct7 {
                    0b0000001 => Inst::Remuw { rd, rs1, rs2 },
                    _ => Inst::Error(inst),
                },
                _ => Inst::Error(inst),
            },

            0b0101111 => match funct3 {
                // ATOMICS, we don't actually do much to support these since the emulator is strictly single threaded.
                0b010 => match funct5 {
                    0b00000 => Inst::Amoaddw { rd, rs1, rs2 },
                    0b00001 => Inst::Amoswapw { rd, rs1, rs2 },
                    0b00010 => Inst::Lrw { rd, rs1 },
                    0b00011 => Inst::Scw { rs2, rs1, rd },
                    0b01000 => Inst::Amoorw { rs2, rs1, rd },
                    0b11100 => Inst::Amomaxuw { rs2, rs1, rd },
                    _ => Inst::Error(inst),
                },
                0b011 => match funct5 {
                    0b00000 => Inst::Amoaddd { rd, rs1, rs2 },
                    0b00001 => Inst::Amoswapd { rd, rs1, rs2 },
                    0b00010 => Inst::Lrd { rd, rs1 },
                    0b00011 => Inst::Scd { rs2, rs1, rd },
                    0b11100 => Inst::Amomaxud { rs2, rs1, rd },
                    _ => Inst::Error(inst),
                },
                _ => Inst::Error(inst),
            },

            // floating point operations
            0b1010011 => {
                let rm = ((inst >> 12) & 0b11) as u8;
                match (funct7, rs2.0, rm) {
                    (0b001101, rs2, _rm) => Inst::Fdivd {
                        rd: FReg(rd.0),
                        rs1: FReg(rs1.0),
                        rs2: FReg(rs2),
                    },
                    (0b1010001, rs2, 0b000) => Inst::Fled {
                        rd,
                        rs1: FReg(rs1.0),
                        rs2: FReg(rs2),
                    },
                    (0b1101001, 0b00011, rm) => Inst::Fcvtdlu {
                        rd,
                        rs1: FReg(rs1.0),
                        rm,
                    },
                    _ => Inst::Error(inst),
                }
            }
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
                    _ => Inst::Error(inst),
                }
            }

            0b1100111 => {
                let offset = (inst & 0xFFF00000) as i32 >> 12;
                match funct3 {
                    0b000 => Inst::Jalr { rd, rs1, offset },
                    _ => Inst::Error(inst),
                }
            }

            0b1101111 => {
                let offset = (inst & 0b11111111000000000000) as i32 // imm[19:12]
                           | ((inst & 0b100000000000000000000) >> 9) as i32 // imm[11]
                           | ((inst & 0b1111111111000000000000000000000) >> 20) as i32 // imm[10:1]
                           | ((inst & 0b10000000000000000000000000000000) as i32) >> 11; // imm[20]

                Inst::Jal { rd, offset }
            }

            0b1110011 => Inst::Ecall,

            _ => Inst::Error(inst),
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
                            imm: imm as i32,
                        }
                    }
                    0b001 => {
                        // C.FLD

                        // nzuimm
                        let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                                | (inst & 0b1100000) << 1; // imm[7:6]
                        let rd = FReg((((inst >> 2) & 0b111) + 8) as u8);
                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);

                        Inst::Fld {
                            rd,
                            rs1,
                            offset: imm as i32,
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
                    0b101 => {
                        // C.FSD
                        let rs2 = Reg((((inst >> 2) & 0b111) + 8) as u8);
                        let rs1 = Reg((((inst >> 7) & 0b111) + 8) as u8);
                        let offset = (inst & 0b1100000) << 1 // imm[7:6]
                                   | (inst & 0b1110000000000) >> 7; // imm[5:3]

                        Inst::Fsd {
                            rs1,
                            rs2: FReg(rs2.0),
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
                    _ => Inst::Error(inst as u32),
                }
            }
            0b01 => {
                match funct3 {
                    0b000 => {
                        // C.ADDI

                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        Inst::Addi {
                            rd,
                            rs1: rd,
                            imm: imm as i32,
                        }
                    }
                    0b001 => {
                        // C.ADDIW

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
                        // C.LI

                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        Inst::Addi {
                            rd,
                            rs1: Reg(0),
                            imm: imm as i32,
                        }
                    }
                    0b011 => {
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);

                        if rd == Reg(2) {
                            // C.ADDI16SP
                            let imm = (((inst & 0b1000000000000) << 3) as i16 >> 6) as i32 // imm[9]
                                    | ((inst & 0b100) << 3) as i32 // imm[5]
                                    | ((inst & 0b11000) << 4) as i32 // imm[8:7]
                                    | ((inst & 0b100000) << 1) as i32 // imm[6]
                                    | ((inst & 0b1000000) >> 2) as i32; // imm[4]

                            Inst::Addi {
                                rd: SP,
                                rs1: SP,
                                imm,
                            }
                        } else {
                            // C.LUI
                            let imm = ((((inst & 0b1000000000000) << 3) as i16 as i32) << 2)  // imm[17]
                                    | ((inst as u32 & 0b1111100) << 10) as i32; // imm[16:12]

                            Inst::Lui { rd, imm }
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

                                assert_ne!(shamt, 0);

                                if shamt == 0 {
                                    Inst::Error(inst as u32)
                                } else {
                                    Inst::Srli {
                                        rd,
                                        rs1: rd,
                                        shamt: shamt as u32,
                                    }
                                }
                            }

                            // C.SRAI
                            0b01 => {
                                let shamt = (inst & 0b1000000000000) >> 7 // imm[5]
                                          | (inst & 0b1111100) >> 2; // imm[4:0]

                                assert_ne!(shamt, 0);

                                if shamt == 0 {
                                    Inst::Error(inst as u32)
                                } else {
                                    Inst::Srai {
                                        rd,
                                        rs1: rd,
                                        shamt: shamt as u32,
                                    }
                                }
                            }

                            // C.ANDI
                            0b10 => {
                                let imm = ((inst & 0b1000000000000) << 3) as i16 >> 10 // imm[5]
                                        | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                                Inst::Andi {
                                    rd,
                                    rs1: rd,
                                    imm: imm as i32,
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
                                    _ => Inst::Error(inst as u32),
                                }
                            }
                            _ => Inst::Error(inst as u32),
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
                            offset: imm as i16 as i32,
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
                    _ => Inst::Error(inst as u32),
                }
            }
            0b10 => {
                match funct3 {
                    0b000 => {
                        // C.SLLI
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);
                        let shamt = (inst & 0b1000000000000) >> 7 // imm[5]
                                  | (inst & 0b1111100) >> 2; // imm[4:0]

                        if shamt != 0 {
                            Inst::Slli {
                                rd,
                                rs1: rd,
                                shamt: shamt as u32,
                            }
                        } else {
                            Inst::Error(inst as u32)
                        }
                    }
                    0b001 => {
                        // C.FLDSP
                        let rd = FReg(((inst >> 7) & 0b11111) as u8);
                        let offset = (inst & 0b1000000000000) >> 7 // imm[5]
                                   | (inst & 0b11100) << 4 // imm[8:6]
                                   | (inst & 0b1100000) >> 2; // imm[4:3]

                        Inst::Fld {
                            rd,
                            rs1: SP,
                            offset: offset as i32,
                        }
                    }
                    0b010 => {
                        let rd = Reg(((inst >> 7) & 0b11111) as u8);
                        let imm = (inst & 0b1100) << 4 // imm[7:6]
                                | (inst & 0b1110000) >> 2 // imm[4:2]
                                | (inst & 0b1000000000000) >> 7; // imm[5]

                        // C.LWSP
                        if rd != Reg(0) {
                            Inst::Lw {
                                rd,
                                rs1: SP,
                                offset: imm as i32,
                            }
                        } else {
                            Inst::Error(inst as u32)
                        }
                    }
                    0b011 => {
                        // C.LDSP

                        let rd = Reg(((inst >> 7) & 0b11111) as u8);
                        let imm = (inst & 0b1000000000000) >> 7 // imm[5]
                                | (inst & 0b11100) << 4 // imm[8:6]
                                | (inst & 0b1100000) >> 2; // imm[4:3]

                        if rd != Reg(0) {
                            Inst::Ld {
                                rd,
                                rs1: SP,
                                offset: imm as i32,
                            }
                        } else {
                            Inst::Error(inst as u32)
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
                        }
                        // C.JALR
                        else if imm == 1 && rs1 != Reg(0) && rs2 == Reg(0) {
                            Inst::Jalr {
                                rd: RA,
                                rs1,
                                offset: 0,
                            }
                        }
                        // C.EBREAK
                        else {
                            Inst::Ebreak
                        }
                    }
                    0b101 => {
                        // C.FSDSP

                        let rs2 = FReg(((inst >> 2) & 0b11111) as u8);
                        let imm = (inst & 0b1110000000) >> 1 // imm[8:6]
                                | (inst & 0b1110000000000) >> 7; // imm[5:3]

                        Inst::Fsd {
                            rs2,
                            rs1: SP,
                            offset: imm as i32,
                        }
                    }
                    0b110 => {
                        // SWSP

                        let imm = (inst & 0b110000000) >> 1 // imm[7:6]
                                | (inst & 0b1111000000000) >> 7; // imm[5:2]
                        let rs2 = Reg(((inst >> 2) & 0b11111) as u8);

                        Inst::Sw {
                            rs2,
                            rs1: SP,
                            offset: imm as i32,
                        }
                    }
                    0b111 => {
                        // C.SDSP - not C.SWSP since we are emulating RV64C
                        let offset = (inst & 0b1110000000) >> 1 // imm[8:6]
                                   | (inst & 0b1110000000000) >> 7; // imm[5:3]

                        let rs2 = Reg(((inst >> 2) & 0b11111) as u8);

                        Inst::Sd {
                            rs1: SP,
                            rs2,
                            offset: offset as i32,
                        }
                    }
                    _ => Inst::Error(inst as u32),
                }
            }
            0b11 => Inst::Error(inst as u32),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register::*;

    #[test]
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

    #[test]
    fn compressed_branch_decoding() {
        let (inst, _) = Inst::decode(0x0000dc85);
        assert_eq!(
            inst,
            Inst::Beq {
                rs1: S1,
                rs2: Reg(0),
                offset: -200,
            }
        );

        let (inst, _) = Inst::decode(0x0000fc85);
        assert_eq!(
            inst,
            Inst::Bne {
                rs1: S1,
                rs2: Reg(0),
                offset: -200,
            }
        );
    }

    #[test]
    fn xori_decoding() {
        let (inst, _) = Inst::decode(0xfff64613);
        assert_eq!(
            inst,
            Inst::Xori {
                rd: A2,
                rs1: A2,
                imm: -1
            },
        );
    }

    #[test]
    fn srliw_srli_decoding() {
        let (inst, _) = Inst::decode(0x0087d49b);
        assert_eq!(
            inst,
            Inst::Srliw {
                rd: S1,
                rs1: A5,
                shamt: 8,
            }
        );

        let (inst, _) = Inst::decode(0x0307d813);
        assert_eq!(
            inst,
            Inst::Srli {
                rd: A6,
                rs1: A5,
                shamt: 48
            }
        );

        let (inst, _) = Inst::decode(0x02091793);
        assert_eq!(
            inst,
            Inst::Slli {
                rd: A5,
                rs1: S2,
                shamt: 32
            }
        );
    }

    #[test]
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
