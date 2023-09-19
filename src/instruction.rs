use crate::SP;

enum Instruction {
    // MISC.
    Fence,
    Ecall,
    Error(Box<str>), // not a risc v instruction but useful for control flow here
    Lui { rd: u8, imm: u32 },

    // LOADS/STORES
    Ld { rd: u8, rs1: u8, offset: i32 },
    Lw { rd: u8, rs1: u8, offset: i32 },
    Lhu { rd: u8, rs1: u8, offset: i32 },
    Lbu { rd: u8, rs1: u8, offset: i32 },
    Sd { rs1: u8, rs2: u8, offset: i32 },
    Sw { rs1: u8, rs2: u8, offset: i32 },
    Sh { rs1: u8, rs2: u8, offset: i32 },
    Sb { rs1: u8, rs2: u8, offset: i32 },

    // MATH OPERATIONS
    Add { rd: u8, rs1: u8, rs2: u8 },
    Addw { rd: u8, rs1: u8, rs2: u8 },
    Addi { rd: u8, rs1: u8, imm: u64 },
    Addiw { rd: u8, rs1: u8, imm: u32 },
    And { rd: u8, rs1: u8, rs2: u8 },
    Andi { rd: u8, rs1: u8, imm: u64 },
    Sub { rd: u8, rs1: u8, rs2: u8 },
    Subw { rd: u8, rs1: u8, rs2: u8 },
    Slli { rd: u8, rs1: u8, shamt: u64 },
    Slliw { rd: u8, rs1: u8, shamt: u32 },
    Srli { rd: u8, rs1: u8, shamt: u64 },
    Or { rd: u8, rs1: u8, rs2: u8 },
    Xor { rd: u8, rs1: u8, rs2: u8 },


    // JUMPING
    Auipc { rd: u8, imm: u64 },
    Jal { rd: u8, offset: u64 },
    Jalr { rd: u8, rs1: u8, offset: u64 },

    // BRANCHES
    Beq { rs1: u8, rs2: u8, offset: i32 },
    Bne { rs1: u8, rs2: u8, offset: i32 },
    Blt { rs1: u8, rs2: u8, offset: i32 },
    Bge { rs1: u8, rs2: u8, offset: i32 },
    Bgeu { rs1: u8, rs2: u8, offset: i32 },
    Bltu { rs1: u8, rs2: u8, offset: i32 },
}

impl Instruction {
    pub fn decode(inst: u32) -> Instruction {
        match inst & 0b11 {
            0b00 | 0b01 | 0b10 => Self::decode_compressed(inst as u16),
            0b11 => Self::decode_normal(inst),
        }
    }

    fn decode_normal(inst: u32) -> Instruction {
        let opcode = inst & 0b1111111;
        let rd = ((inst >> 7) & 0b11111) as u8;
        let rs1 = ((inst >> 15) & 0b11111) as u8;
        let rs2 = ((inst >> 20) & 0b11111) as u8;
        let funct3 = (inst >> 12) & 0b111;

        match opcode {
            0b0000011 => {
                let offset = ((inst & 0xFFF00000) as i32) >> 20;

                match funct3 {
                    0b010 => Instruction::Lw { rd, rs1, offset },
                    0b011 => Instruction::Ld { rd, rs1, offset },
                    0b100 => Instruction::Lbu { rd, rs1, offset },
                    0b101 => Instruction::Lhu { rd, rs1, offset },
                    _ => Instruction::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
                }
            }
            0b0001111 => Instruction::Fence,
            0b0010011 => {
                let imm = ((inst & 0xFFF00000) as i32 as i64 >> 20) as u64;
                match funct3 {
                    0b000 => Instruction::Addi { rd, rs1, imm },
                    0b001 => {
                        let shamt = ((inst >> 20) & 0b11111) as u64;
                        Instruction::Slli { rd, rs1, shamt }
                    }
                    0b111 => Instruction::Andi { rd, rs1, imm },
                    _ => Instruction::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
                }
            }

            // AUIPC - Add Upper Immediate to PC
            0b0010111 => {
                let imm = (inst & 0xFFFFF000) as i32 as i64 as u64;
                Instruction::Auipc { rd, imm }
            }

            0b0011011 => match funct3 {
                0b000 => {
                    let imm = ((inst & 0b11111111111100000000000000000000) as i32 >> 20) as u32;
                    Instruction::Addiw { rd, rs1, imm }
                }
                0b001 => {
                    let shamt = ((inst >> 20) & 0b111111) as u32;
                    Instruction::Slliw { rd, rs1, shamt }
                }
                _ => Instruction::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
            },

            // STORE
            0b0100011 => {
                let offset = ((inst & 0b11111110000000000000000000000000) as i32) >> 20 // imm[11:5]
                           | (inst & 0b111110000000) as i32 >> 7; // imm[4:0]

                match funct3 {
                    0b011 => Instruction::Sd { rs1, rs2, offset }
                    0b010 => Instruction::Sw { rs1, rs2, offset },
                    0b001 => Instruction::Sh { rs1, rs2, offset },
                    0b000 => Instruction::Sb { rs1, rs2, offset },
                    _ => Instruction::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
                }
            }

            0b0110011 => Instruction::Add { rd, rs1, rs2 },

            // Branches
            0b1100011 => {
                let offset = ((inst & 0b1111110000000000000000000000000) >> 20) as i32  // imm[10:5]
                           | ((inst & 0b10000000000000000000000000000000) as i32 >> 19) // imm[12]
                           | ((inst & 0b10000000) << 4) as i32 // imm[11]
                           | ((inst & 0b111100000000) >> 7) as i32; // imm[4:1]

                match funct3 {
                    0b000 => Instruction::Beq { rs1, rs2, offset },
                    0b001 => Instruction::Bne { rs1, rs2, offset },
                    0b100 => Instruction::Blt { rs1, rs2, offset },
                    0b101 => Instruction::Bge { rs1, rs2, offset },
                    0b110 => Instruction::Bltu { rs1, rs2, offset },
                    0b111 => Instruction::Bgeu { rs1, rs2, offset },
                    _ => Instruction::Error(format!("unimplemented: {opcode:b} funct3={funct3:b}").into()),
                }
            }

            0b1101111 => {
                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let offset = (((inst & 0x80000000) as i32 as i64 >> 11) as u64) // imm[20]
                           | (inst & 0xff000) as u64 // imm[19:12]
                           | ((inst >> 9) & 0x800) as u64 // imm[11]
                           | ((inst >> 20) & 0x7fe) as u64; // imm[10:1]

                Instruction::Jal { rd, offset }
            }

            0b1110011 => Instruction::Ecall,

            _ => Instruction::Error(format!("unimplemented: {opcode:b}").into()),
        }
    }

    fn decode_compressed(inst: u16) -> Instruction {
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
                        let rd = (((inst >> 2) & 0b111) + 8) as u8;

                        Instruction::Addi { rd, rs1: SP as u8, imm: imm as u64 }
                    }
                    0b010 => {
                        // C.LW
                        let rd = (((inst >> 2) & 0b111) + 8) as u8;
                        let rs1 = (((inst >> 7) & 0b111) + 8) as u8;
                        let offset = (inst & 0b100000) << 1 // imm[6]
                                   | (inst & 0b1000000) >> 4 // imm[2]
                                   | (inst & 0b1110000000000) >> 7; // imm[5:3]

                        Instruction::Lw { rd, rs1, offset: offset as i32 }
                    }
                    0b011 => {
                        // C.LD
                        let rd = (((inst >> 2) & 0b111) + 8) as u8;
                        let rs1 = (((inst >> 7) & 0b111) + 8) as u8;
                        let offset = ((inst >> 7) & 0b111000) | (((inst >> 5) & 0b111) << 6);

                        Instruction::Ld { rd, rs1, offset: offset as i32 }
                    }
                    0b110 => {
                        // C.SW
                        let rs1 = (((inst >> 7) & 0b111) + 8) as u8;
                        let rs2 = (((inst >> 2) & 0b111) + 8) as u8;
                        let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                                | (inst & 0b100000) << 1 // imm[6]
                                | (inst & 0b1000000) >> 4; // imm[2]

                        Instruction::Sw { rs1, rs2, offset: imm as i32 }
                    }
                    0b111 => {
                        // C.SD
                        let rs1 = (((inst >> 7) & 0b111) + 8) as u8;
                        let rs2 = (((inst >> 2) & 0b111) + 8) as u8;
                        let imm = (inst & 0b1110000000000) >> 7 // imm[5:3]
                                | (inst & 0b1100000) << 1; // imm[7:6]

                        Instruction::Sd { rs1, rs2, offset: imm as i32 }
                    }
                    _ => Instruction::Error(format!("unimplemented: quadrant={quadrant:02b} {funct3:03b}").into()),
                }
            }
            0b01 => {
                match funct3 {
                    0b000 => {
                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = ((inst >> 7) & 0b11111) as u8;

                        Instruction::Addi { rd, rs1: rd, imm: imm as u64 }
                    }
                    0b001 => {
                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = ((inst >> 7) & 0b11111) as u8;

                        Instruction::Addiw { rd, rs1: rd, imm: imm as u32 }
                    }
                    0b010 => {
                        let imm = (((inst & 0b1000000000000) << 3) as i16 >> 10) // imm[5]
                                | (inst & 0b1111100) as i16 >> 2; // imm[4:0]
                        let rd = ((inst >> 7) & 0b11111) as u8;

                        Instruction::Addi { rd, rs1: 0, imm: imm as u64 }
                    }
                    0b011 => {
                        let rd = ((inst >> 7) & 0b11111) as u8;

                        if rd == 2 {
                            // C.ADDI16SP
                            let imm = (((inst & 0b1000000000000) << 3) as i16 >> 6) as u64 // imm[9]
                                    | ((inst & 0b100) << 3) as u64 // imm[5]
                                    | ((inst & 0b11000) << 4) as u64 // imm[8:7]
                                    | ((inst & 0b100000) << 1) as u64 // imm[6]
                                    | ((inst & 0b1000000) >> 2) as u64; // imm[4]

                            Instruction::Addi { rd: SP as u8, rs1: SP as u8, imm }
                        } else {
                            // C.LUI
                            let imm = ((((inst & 0b1000000000000) << 3) as i16 as i32) << 2)  // imm[17]
                                    | ((inst as u32 & 0b1111100) << 10) as i32; // imm[16:12]

                            Instruction::Lui { rd, imm: imm as u32 }
                        }
                    }
                    0b100 => {
                        let funct2 = (inst >> 10) & 0b11;
                        let rd = (((inst >> 7) & 0b111) + 8) as u8;

                        match funct2 {
                            // C.SRLI
                            0b00 => {
                                let shamt = (inst & 0b1000000000000) >> 7 // imm[5]
                                          | (inst & 0b1111100) >> 2; // imm[4:0]

                                if shamt == 0 {
                                    panic!("Immediate must be nonzero");
                                }

                                Instruction::Srli { rd, rs1: rd, shamt: shamt as u64 }
                            }

                            // C.ANDI
                            0b10 => {
                                let imm = ((inst & 0b1000000000000) << 3) as i16 >> 10 // imm[5]
                                        | (inst & 0b1111100) as i16 >> 2; // imm[4:0]

                                Instruction::Andi { rd, rs1: rd, imm: imm as u64 }
                            }

                            0b11 => {
                                let funct3 = (((inst >> 12) & 0b1) << 2) | (inst >> 5) & 0b11;
                                let rs2 = (((inst >> 2) & 0b111) + 8) as u8;

                                match funct3 {
                                    0b000 => Instruction::Sub { rd, rs1: rd, rs2 }
                                    0b001 => Instruction::Xor { rd, rs1: rd, rs2 },
                                    0b010 => Instruction::Or { rd, rs1: rd, rs2 },
                                    0b011 => Instruction::And { rd, rs1: rd, rs2 },
                                    0b100 => Instruction::Subw { rd, rs1: rd, rs2 },
                                    0b101 => Instruction::Addw { rd, rs1: rd, rs2 },

                                    _ => {
                                        unreachable!();
                                    }
                                }
                            }
                            _ => Instruction::Error(format!("unimplemented instruction: {inst:0b} {funct2:0b}").into())
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

                        Instruction::Jal { rd: 0, offset: imm as i16 as u64 }
                    }
                    0b110 => {
                        // C.BEQZ
                        let offset = ((inst & 0b110000000000) >> 7) as i32 // imm[4:3]
                                   | (((inst & 0b1000000000000) >> 5) as i8 as i32) << 1 // imm[8]
                                   | ((inst & 0b100) << 3) as i32 // imm[5]
                                   | ((inst & 0b11000) >> 2) as i32 // imm[2:1]
                                   | ((inst & 0b1100000) << 1) as i32; // imm[7:6]

                        let rs1 = (((inst >> 7) & 0b111) + 8) as u8;

                        Instruction::Beq { rs1, rs2: 0, offset }
                    }
                    0b111 => {
                        // C.BNEZ
                        let offset = ((inst & 0b110000000000) >> 7) as i32 // imm[4:3]
                                   | (((inst & 0b1000000000000) >> 5) as i8 as i32) << 1 // imm[8]
                                   | ((inst & 0b100) << 3) as i32 // imm[5]
                                   | ((inst & 0b11000) >> 2) as i32 // imm[2:1]
                                   | ((inst & 0b1100000) << 1) as i32; // imm[7:6]

                        let rs1 = (((inst >> 7) & 0b111) + 8) as u8;

                        Instruction::Bne { rs1, rs2: 0, offset }
                    }
                    _ => Instruction::Error(format!("unimplemented: quadrant={quadrant:02b} {funct3:03b}").into()),
                }
            }
            0b10 => {
                match funct3 {
                    0b000 => {
                        let shamt = ((((inst >> 12) & 0b1) << 5) | ((inst >> 2) & 0b1111)) as u64;
                        let rd = ((inst >> 7) & 0b11111) as u8;
                        Instruction::Slli { rd, rs1: rd, shamt }
                    }
                    0b011 => {
                        let rd = ((inst >> 7) & 0b11111) as u8;
                        let imm = (inst & 0b1000000000000) >> 7 // imm[5]
                                | (inst & 0b11100) << 4 // imm[8:6]
                                | (inst & 0b1100000) >> 2; // imm[4:3]

                        if rd != 0 {
                            // C.LDSP
                            Instruction::Ld { rd, rs1: SP as u8, offset: imm as i32 }
                        } else {
                            panic!("C.FLWSP not implemented");
                        }
                    }
                    0b100 => {
                        let imm = (inst >> 12) & 0b1;
                        let rs1 = ((inst >> 7) & 0b11111) as u8;
                        let rs2 = ((inst >> 2) & 0b11111) as u8;

                        // C.JR - ret
                        if imm == 0 && rs1 != 0 && rs2 == 0 {
                            Instruction::Jalr { rd: 0, rs1, offset: 0 }
                        }
                        // C.MV - Move
                        else if imm == 0 && rs1 != 0 && rs2 != 0 {
                            Instruction::Add { rd: rs1, rs1: 0, rs2 }
                        }
                        // C.ADD - Add
                        else if imm == 1 && rs1 != 0 && rs2 != 0 {
                            Instruction::Add { rd: rs1, rs1, rs2 }
                        } else {
                            Instruction::Error(format!("compressed instruction `{inst:016b}` not implemented.").into())
                        }
                    }
                    0b111 => {
                        // C.SDSP - not C.SWSP since we are emulating RV64C
                        let offset = (((inst >> 7) & 0b111000) | ((inst >> 1) & 0b111000000)) as i32;
                        let rs1 = ((inst >> 2) & 0b11111) as u8;

                        Instruction::Sd { rs1, rs2: SP as u8, offset }
                    }
                    _ => Instruction::Error(format!("quadrant={quadrant:02b} funct3={funct3:03b}").into())
                }
            }
            0b11 => Instruction::Error("Quadrant 11 should not exist".into())
            _ => unreachable!(),
        }
    }
}
