use std::collections::HashSet;

use crate::{
    cache::Cache,
    register::{FReg, Reg},
};

pub const CACHE_SIZE: u64 = 0x500;

#[derive(Clone, Debug)]
pub struct Profiler {
    x_pipeline_delay: [u64; 32],
    f_pipeline_delay: [u64; 32],

    pub cycle_count: u64,
    pub cache_hit_count: u64,
    pub cache_miss_count: u64,
    pub mispredicted_branch_count: u64,
    pub predicted_branch_count: u64,

    // by default, we assume the branch is not taken.
    // if the address of the branch instruction is inside
    // this hashmap, we take the branch
    branch_predictor: Cache<u64, bool, 100>,

    // stores the address of the most recently accessed memory location
    // used to calculate cache hits/misses
    last_mem_access: u64,

    pub running: bool,
    ignore_dynamic_linker_instructions: bool,
}

impl Profiler {
    pub fn new() -> Profiler {
        Profiler {
            x_pipeline_delay: [0; 32],
            f_pipeline_delay: [0; 32],

            cycle_count: 0,
            cache_hit_count: 0,
            cache_miss_count: 0,
            mispredicted_branch_count: 0,
            predicted_branch_count: 0,
            branch_predictor: Cache::new(),
            last_mem_access: 0,
            running: false,
            ignore_dynamic_linker_instructions: true,
        }
    }

    pub fn tick(&mut self, pc: u64) {
        if self.is_counted(pc) {
            self.cycle_count += 1;
        }
    }

    #[inline]
    fn is_counted(&self, pc: u64) -> bool {
        self.running && !(self.ignore_dynamic_linker_instructions && pc >> 56 == 2)
    }

    #[inline]
    pub fn pipeline_stall_xx(&mut self, reg1: Reg, reg2: Reg, pc: u64) {
        if self.is_counted(pc) {
            self.cycle_count = self
                .cycle_count
                .max(self.x_pipeline_delay[reg1])
                .max(self.x_pipeline_delay[reg2]);
        }
    }

    #[inline]
    pub fn pipeline_stall_xf(&mut self, reg1: Reg, reg2: FReg, pc: u64) {
        if self.is_counted(pc) {
            self.cycle_count = self
                .cycle_count
                .max(self.x_pipeline_delay[reg1])
                .max(self.f_pipeline_delay[reg2.0 as usize]);
        }
    }

    #[inline]
    pub fn pipeline_stall_x(&mut self, reg1: Reg, pc: u64) {
        if self.is_counted(pc) {
            self.cycle_count = self.cycle_count.max(self.x_pipeline_delay[reg1]);
        }
    }

    #[inline]
    pub fn branch_taken(&mut self, pc: u64) {
        if self.is_counted(pc) {
            match self.branch_predictor.update(pc, true) {
                None | Some(false) => {
                    // mispredicted branch incurs a 4 cycle penalty
                    self.mispredicted_branch_count += 1;
                    self.cycle_count += 4;
                }
                Some(true) => {
                    self.predicted_branch_count += 1;
                }
            }
        }
    }

    #[inline]
    pub fn branch_not_taken(&mut self, pc: u64) {
        if self.is_counted(pc) {
            match self.branch_predictor.update(pc, false) {
                Some(true) => {
                    // mispredicted branch incurs a 4 cycle penalty
                    self.mispredicted_branch_count += 1;
                    self.cycle_count += 4;
                }
                None | Some(false) => {
                    self.predicted_branch_count += 1;
                }
            }
        }
    }

    #[inline]
    pub fn add_delay_x(&mut self, reg: Reg, amount: u64) {
        self.x_pipeline_delay[reg] = self.cycle_count + amount;
    }

    pub fn add_load_delay_f(&mut self, rd: FReg, addr: u64, pc: u64) {
        if self.is_counted(pc) {
            // if cache hit, 3 cycle delay
            if self.last_mem_access.abs_diff(addr) < CACHE_SIZE {
                self.cache_hit_count += 1;
                self.f_pipeline_delay[rd.0 as usize] = self.cycle_count + 3;
            }
            // if cache miss, 200 cycle delay
            else {
                self.cache_miss_count += 1;
                self.f_pipeline_delay[rd.0 as usize] = self.cycle_count + 200;
            }

            self.last_mem_access = addr;
        }
    }

    pub fn add_load_delay_x(&mut self, rd: Reg, addr: u64, pc: u64) {
        if self.is_counted(pc) {
            // if cache hit, 3 cycle delay
            if self.last_mem_access.abs_diff(addr) < CACHE_SIZE {
                self.cache_hit_count += 1;
                self.x_pipeline_delay[rd] = self.cycle_count + 3;
            }
            // if cache miss, 200 cycle delay
            else {
                self.cache_miss_count += 1;
                self.x_pipeline_delay[rd] = self.cycle_count + 200;
            }

            self.last_mem_access = addr;
        }
    }
}
