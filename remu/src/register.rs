#![allow(unused)]

use std::{
    fmt::Display,
    ops::{Index, IndexMut},
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

pub const RA: Reg = Reg(1);
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
