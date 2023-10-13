use std::{collections::HashMap, mem, num::NonZeroU64};

use dynasm::dynasm;
use dynasmrt::{x64::Assembler, AssemblyOffset, DynasmApi, DynasmLabelApi, ExecutableBuffer};

use crate::{
    instruction::Inst,
    profiler::Profiler,
    register::{Reg, RA},
    system::Emulator,
};

macro_rules! my_dynasm {
    ($ops:ident $($t:tt)*) => {
        dynasm!($ops
            ; .arch x64
            ; .alias a_emu, rcx
            ; .alias a_pc, rdx
            ; .alias a_registers, r8
            $($t)*
        )
    }
}

macro_rules! load_reg {
    ($ops:ident, $store_loc:ident <= $reg:expr) => {
        my_dynasm!($ops
            ; mov $store_loc, QWORD [a_registers + (8 * $reg.0 as i32)]
        )
    };
}

macro_rules! store_reg {
    ($ops:ident, $out_reg:ident => $reg:expr) => {
        my_dynasm!($ops
            ; mov QWORD [a_registers + (8 * $reg.0 as i32)], $out_reg
        )
    };
}

macro_rules! call_extern {
    ($ops:ident, $addr:expr) => {my_dynasm!($ops
        ; mov rax, QWORD $addr as _
        ; call rax
        ; mov rcx, [rsp + 0x30]
        ; mov rdx, [rsp + 0x38]
        ; mov r8,  [rsp + 0x40]
        ; mov r9,  [rsp + 0x48]
    );};
}

macro_rules! pipeline_stall {
    ($ops:ident, x . $r1:expr) => {
        my_dynasm!($ops
            ; mov rdx, $r1.0 as _
            ;; call_extern!($ops, profiler_pipeline_stall_x)
        );
    };

    ($ops:ident, x . $r1:expr, x . $r2:expr) => {
        my_dynasm!($ops
            ; mov rdx, $r1.0 as _
            ; mov r8, $r2.0 as _
            ;; call_extern!($ops, profiler_pipeline_stall_xx)
        );
    };
}

macro_rules! branch_impl {
    ($btype:ident : $ops:ident, $profile:expr, $dynamic_labels:expr, $pc:expr, $rs1:expr, $rs2:expr, $offset:expr) => {
        let branch_not_taken_label = $ops.new_dynamic_label();
        my_dynasm!($ops
            ;; if $profile { pipeline_stall!($ops, x.$rs1, x.$rs2); }

            ;; load_reg!($ops, r9 <= $rs1)
            ;; load_reg!($ops, r10 <= $rs2)
            ; cmp r9, r10
            ; $btype =>branch_not_taken_label
            ;; if $profile { call_extern!($ops, branch_taken); }
            ; mov r9, [a_pc]
            ; add r9, $offset
            ; mov [a_pc], r9

            ; mov r9, a_emu => Emulator.inst_counter
            ; add r9, 1
            ; mov a_emu => Emulator.inst_counter, r9

            ; jmp =>$dynamic_labels[&$pc.wrapping_add($offset as u64)]
            ;=>branch_not_taken_label
            ;; if $profile { call_extern!($ops, branch_not_taken); }
        );
    }
}

/// assumes rdx contains offset already, because that's necessary for the load_{size} calls
macro_rules! add_load_delay {
    ($ops:ident, $rd:ident) => {
        my_dynasm!($ops
            ; mov r8, $rd.0 as _
            ;; call_extern!($ops, add_load_delay_x)
        );
    };
}

unsafe extern "win64" fn add_load_delay_x(emu: *mut Emulator, addr: u64, rd: Reg) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.add_load_delay_x(rd, addr, emulator.pc);
}

unsafe extern "win64" fn profiler_tick(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.tick(emulator.pc);
}

unsafe extern "win64" fn profiler_pipeline_stall_xx(emu: *mut Emulator, reg1: Reg, reg2: Reg) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.pipeline_stall_xx(reg1, reg2, emulator.pc);
    emulator.profiler.tick(emulator.pc);
}

unsafe extern "win64" fn profiler_pipeline_stall_x(emu: *mut Emulator, reg1: Reg) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.pipeline_stall_x(reg1, emulator.pc);
    emulator.profiler.tick(emulator.pc);
}

/// returns false if the syscall fails, otherwise true
unsafe extern "win64" fn syscall(emu: *mut Emulator) -> bool {
    let emulator = unsafe { &mut *emu };
    emulator.syscall().is_ok()
}

unsafe extern "win64" fn execute_block(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    emulator.execute_block().expect("Failed to execute block");
}

unsafe extern "win64" fn branch_not_taken(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.branch_not_taken(emulator.pc);
}

unsafe extern "win64" fn branch_taken(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.branch_taken(emulator.pc);
}

unsafe extern "win64" fn store_u64(emu: *mut Emulator, offset: u64, rs2: u64) {
    let emulator = unsafe { &mut *emu };
    emulator
        .memory
        .store::<u64>(offset, rs2)
        .expect("Failed to store");
}

unsafe extern "win64" fn load_u64(emu: *mut Emulator, offset: u64) -> u64 {
    let emulator = unsafe { &mut *emu };
    emulator.memory.load(offset).expect("Failed to store")
}

unsafe extern "win64" fn start_profile(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.running = true;
}

unsafe extern "win64" fn end_profile(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    emulator.profiler.running = false;
}

unsafe extern "win64" fn debug_print_registers(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    println!("{}", emulator.print_registers());
}

unsafe extern "win64" fn log_inst(emu: *mut Emulator) {
    let emulator = unsafe { &mut *emu };
    let inst_data = emulator
        .memory
        .load::<u32>(emulator.pc)
        .expect("Failed to load instruction");
    let (inst, _step) = Inst::decode(inst_data);

    println!("{}", inst.fmt(emulator.pc));
}

const ZERO: i32 = 0;

/// stores a jit recompiled version of a RISC-V function
///
/// the jit compilation block is given 3 arguments:
/// - rcx/emu: *mut Emulator
/// - rdx/pc: *mut u64
/// - r8x/registers: *mut u64
pub struct RVFunction {
    code: ExecutableBuffer,
    start: AssemblyOffset,
}

impl RVFunction {
    pub fn run(&self, emulator: &mut Emulator) {
        // arguments: emulator, pc, x registers
        let func: extern "win64" fn(*mut Emulator, *mut u64, *mut u64) =
            unsafe { mem::transmute(self.code.ptr(self.start)) };

        // emulator
        let emu = emulator as *mut Emulator;

        // pc
        let pc = &mut emulator.pc;

        // x
        let x = emulator.x.as_mut_ptr();

        func(emu, pc, x);
    }

    /// compiles function starting at current pc, until the `ret` instruction is reached
    pub fn compile(emulator: &mut Emulator, profile: bool) -> RVFunction {
        log::debug!("COMPILING FUNCTION {:x}", emulator.pc);

        let mut ops = Assembler::new().expect("Failed to create assembler");
        let start = ops.offset();

        // prepass
        let mut pc = emulator.pc;
        let mut instructions = Vec::new();
        let mut dynamic_labels = HashMap::new();

        let mut done = false;
        while !done {
            let inst_data = emulator
                .memory
                .load::<u32>(pc)
                .expect("Failed to load instruction");
            let (inst, step) = Inst::decode(inst_data);

            match inst {
                Inst::Error(inst) => {
                    // 0 marks end, maybe, who knows
                    if inst == 0 {
                        break;
                    } else {
                        panic!("Invalid instruction: {inst}");
                    }
                }

                // technically JALR could be used for an intra-function jump, but in practice no
                // code generator will do this (or at least I hope)
                Inst::Jalr { rd, rs1, offset } => {
                    // match ret, end of function to stop jit compiling
                    if rd == Reg(0) && rs1 == RA && offset == 0 {
                        done = true;
                    }
                }

                _ => {}
            }

            // create dynamic label for each instruction to allow branches to work
            instructions.push((inst, step));
            dynamic_labels.insert(pc, ops.new_dynamic_label());

            pc += step as u64;
        }

        my_dynasm!(ops
            ; sub rsp, 0x28
            ; mov [rsp + 0x30], rcx
            ; mov [rsp + 0x38], rdx
            ; mov [rsp + 0x40], r8
            ; mov [rsp + 0x48], r9
        );

        let mut started_profile = false;

        let mut pc = emulator.pc;

        for (inst, step) in instructions {
            log::debug!("{pc:16x} {}", inst.fmt(pc));

            let current_label = *dynamic_labels
                .get(&pc)
                .expect("Error getting dynamic label");

            my_dynasm!(ops
                ;=>current_label
                // ;; call_extern!(ops, log_inst)
            );

            if NonZeroU64::new(pc) == emulator.profile_start_point {
                started_profile = true;
                call_extern!(ops, start_profile);
            }

            match inst {
                Inst::Fence => {} // noop
                Inst::Ecall => {
                    call_extern!(ops, syscall);
                }
                Inst::Ebreak => {} // noop
                Inst::Error(e) => {
                    log::error!("{e}");
                }
                Inst::Lui { rd, imm } => {
                    my_dynasm!(ops
                        ;; if profile { call_extern!(ops, profiler_tick); }

                        ; mov r9, imm
                        ;; store_reg!(ops, r9 => rd)
                    );
                }
                Inst::Ld { rd, rs1, offset } => {
                    my_dynasm!(ops
                        ;; if profile {
                            my_dynasm!(ops
                                ;; pipeline_stall!(ops, x.rs1)

                                ;; load_reg!(ops, rdx <= rs1)
                                ; add rdx, offset
                                ;; add_load_delay!(ops, rd)
                            );
                        }

                        ;; load_reg!(ops, rdx <= rs1)
                        ; add rdx, offset

                        ;; call_extern!(ops, load_u64)
                        ;; store_reg!(ops, rax => rd)
                    );
                }
                Inst::Lw { rd, rs1, offset } => todo!(),
                Inst::Lwu { rd, rs1, offset } => todo!(),
                Inst::Lhu { rd, rs1, offset } => todo!(),
                Inst::Lb { rd, rs1, offset } => todo!(),
                Inst::Lbu { rd, rs1, offset } => todo!(),
                Inst::Sd { rs1, rs2, offset } => {
                    my_dynasm!(ops
                        ;; if profile { pipeline_stall!(ops, x.rs1, x.rs2); }

                        ;; load_reg!(ops, rdx <= rs1)
                        ;; load_reg!(ops, r8 <= rs2)
                        ; add rdx, offset
                        ;; call_extern!(ops, store_u64)
                    );
                }
                Inst::Sw { rs1, rs2, offset } => todo!(),
                Inst::Sh { rs1, rs2, offset } => todo!(),
                Inst::Sb { rs1, rs2, offset } => todo!(),
                Inst::Add { rd, rs1, rs2 } => {
                    my_dynasm!(ops
                        ;; if profile { pipeline_stall!(ops, x.rs1, x.rs2); }

                        ;; load_reg!(ops, r9 <= rs1)
                        ;; load_reg!(ops, r10 <= rs2)
                        ; add r9, r10
                        ;; store_reg!(ops, r9 => rd)
                    );
                }
                Inst::Addw { rd, rs1, rs2 } => {
                    my_dynasm!(ops
                        ;; if profile { pipeline_stall!(ops, x.rs1, x.rs2); }

                        ;; load_reg!(ops, r9 <= rs1)
                        ;; load_reg!(ops, r10 <= rs2)
                        ; add r9d, r10d
                        ;; store_reg!(ops, r9 => rd)
                    );
                }
                Inst::Addi { rd, rs1, imm } => {
                    my_dynasm!(ops
                        ;; if profile { pipeline_stall!(ops, x.rs1); }

                        ;; load_reg!(ops, r9 <= rs1)
                        ; add r9, imm
                        ;; store_reg!(ops, r9 => rd)
                    );
                }
                Inst::Addiw { rd, rs1, imm } => {
                    my_dynasm!(ops
                        ;; if profile { pipeline_stall!(ops, x.rs1); }

                        ;; load_reg!(ops, r9 <= rs1)
                        ; add r9d, imm
                        ;; store_reg!(ops, r9 => rd)
                    );
                }
                Inst::Div { rd, rs1, rs2 } => todo!(),
                Inst::Divw { rd, rs1, rs2 } => todo!(),
                Inst::Divu { rd, rs1, rs2 } => todo!(),
                Inst::Divuw { rd, rs1, rs2 } => todo!(),
                Inst::And { rd, rs1, rs2 } => todo!(),
                Inst::Andi { rd, rs1, imm } => todo!(),
                Inst::Sub { rd, rs1, rs2 } => todo!(),
                Inst::Subw { rd, rs1, rs2 } => todo!(),
                Inst::Sll { rd, rs1, rs2 } => todo!(),
                Inst::Sllw { rd, rs1, rs2 } => todo!(),
                Inst::Slli { rd, rs1, shamt } => todo!(),
                Inst::Slliw { rd, rs1, shamt } => todo!(),
                Inst::Srl { rd, rs1, rs2 } => todo!(),
                Inst::Srlw { rd, rs1, rs2 } => todo!(),
                Inst::Srli { rd, rs1, shamt } => todo!(),
                Inst::Srliw { rd, rs1, shamt } => todo!(),
                Inst::Sra { rd, rs1, rs2 } => todo!(),
                Inst::Sraw { rd, rs1, rs2 } => todo!(),
                Inst::Srai { rd, rs1, shamt } => todo!(),
                Inst::Sraiw { rd, rs1, shamt } => todo!(),
                Inst::Or { rd, rs1, rs2 } => todo!(),
                Inst::Ori { rd, rs1, imm } => todo!(),
                Inst::Xor { rd, rs1, rs2 } => todo!(),
                Inst::Xori { rd, rs1, imm } => todo!(),
                Inst::Auipc { rd, imm } => todo!(),
                Inst::Jal { rd, offset } => {
                    my_dynasm!(ops
                        ;; if profile { call_extern!(ops, profiler_tick); }

                        // store pc in rd
                        ;; if rd.0 != 0 {
                            my_dynasm!(ops
                                ; mov r9, [a_pc]
                                ; add r9, step as _
                                ;; store_reg!(ops, r9 => rd)
                            );
                        }

                        // set pc to new address
                        ; add [a_pc], offset as _

                        // actually start executing that new function in the emulator
                        ;; call_extern!(ops, execute_block)

                        ; sub [a_pc], step as _
                    );
                }
                Inst::Jalr { rd, rs1, offset } => {
                    my_dynasm!(ops
                        ;; if profile { pipeline_stall!(ops, x.rs1); }

                        ;; if rd.0 != 0 {
                            my_dynasm!(ops
                                ; mov r9, [a_pc]
                                ; add r9, step as _
                                ;; store_reg!(ops, r9 => rd)
                            );
                        }

                        // set pc to new address
                        ;; load_reg!(ops, r10 <= rs1)
                        ; add r10, offset as _
                        ; sub r10, step as _
                        ; mov [a_pc], r10
                    );
                }
                Inst::Beq { rs1, rs2, offset } => {
                    branch_impl!(jne :
                        ops, profile, dynamic_labels, pc, rs1, rs2, offset);
                }
                Inst::Bne { rs1, rs2, offset } => {
                    branch_impl!(je :
                        ops, profile, dynamic_labels, pc, rs1, rs2, offset);
                }
                Inst::Blt { rs1, rs2, offset } => {
                    branch_impl!(jge :
                        ops, profile, dynamic_labels, pc, rs1, rs2, offset);
                }
                Inst::Bltu { rs1, rs2, offset } => {
                    branch_impl!(jae :
                        ops, profile, dynamic_labels, pc, rs1, rs2, offset);
                }
                Inst::Bge { rs1, rs2, offset } => {
                    branch_impl!(jl :
                        ops, profile, dynamic_labels, pc, rs1, rs2, offset);
                }
                Inst::Bgeu { rs1, rs2, offset } => {
                    branch_impl!(jb :
                        ops, profile, dynamic_labels, pc, rs1, rs2, offset);
                }
                Inst::Mul { rd, rs1, rs2 } => todo!(),
                Inst::Mulhu { rd, rs1, rs2 } => todo!(),
                Inst::Remw { rd, rs1, rs2 } => todo!(),
                Inst::Remu { rd, rs1, rs2 } => todo!(),
                Inst::Remuw { rd, rs1, rs2 } => todo!(),
                Inst::Slt { rd, rs1, rs2 } => todo!(),
                Inst::Sltu { rd, rs1, rs2 } => todo!(),
                Inst::Slti { rd, rs1, imm } => todo!(),
                Inst::Sltiu { rd, rs1, imm } => todo!(),
                Inst::Amoswapw { rd, rs1, rs2 } => todo!(),
                Inst::Amoswapd { rd, rs1, rs2 } => todo!(),
                Inst::Amoaddw { rd, rs1, rs2 } => todo!(),
                Inst::Amoaddd { rd, rs1, rs2 } => todo!(),
                Inst::Amoorw { rd, rs1, rs2 } => todo!(),
                Inst::Amomaxuw { rd, rs1, rs2 } => todo!(),
                Inst::Amomaxud { rd, rs1, rs2 } => todo!(),
                Inst::Lrw { rd, rs1 } => todo!(),
                Inst::Lrd { rd, rs1 } => todo!(),
                Inst::Scw { rd, rs1, rs2 } => todo!(),
                Inst::Scd { rd, rs1, rs2 } => todo!(),
                Inst::Fsd { rs1, rs2, offset } => todo!(),
                Inst::Fsw { rs1, rs2, offset } => todo!(),
                Inst::Fld { rd, rs1, offset } => todo!(),
                Inst::Flw { rd, rs1, offset } => todo!(),
                Inst::Fcvtdlu { rd, rs1, rm } => todo!(),
                Inst::Fcvtds { rd, rs1, rm } => todo!(),
                Inst::Fled { rd, rs1, rs2 } => todo!(),
                Inst::Fdivd { rd, rs1, rs2 } => todo!(),
            }

            // increment pc
            pc += step as u64;
            my_dynasm!(ops
                // set x0 to zero
                ;; store_reg!(ops, ZERO => Reg(0))

                // increment program counter
                ; add [a_pc], step as _

                // increment instruction counter
                ; mov r9, a_emu => Emulator.inst_counter
                ; add r9, 1
                ; mov a_emu => Emulator.inst_counter, r9
            );
        }

        // end of function
        if started_profile {
            call_extern!(ops, end_profile);
        }

        my_dynasm!(ops
            ; add rsp, 0x28
            ; ret
        );

        let code = ops.finalize().unwrap();

        RVFunction { code, start }
    }
}
