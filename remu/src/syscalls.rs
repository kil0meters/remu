// initially taken from this inaccurate blog post:
// https://jborza.com/post/2021-05-11-riscv-linux-syscalls/
// then some edits made

use num_derive::FromPrimitive;

#[derive(FromPrimitive, Debug)]
pub enum Syscall {
    Faccessat = 48,
    Openat = 56,
    Close = 57,
    Lseek = 62,
    Read = 63,
    Write = 64,
    Writev = 66,
    Readlinkat = 78,
    Newfstatat = 79,
    Exit = 93,
    ExitGroup = 94,
    SetTidAddress = 96,
    Futex = 98,
    SetRobustList = 99,
    ClockGettime = 113,
    SchedYield = 124,
    Tgkill = 131,
    RtSigaction = 134,
    RtSigprocmask = 135,
    Getpid = 172,
    Gettid = 178,
    Brk = 214,
    Munmap = 215,
    Mmap = 222,
    Mprotect = 226,
    Prlimit64 = 261,
    Getrandom = 278,
}
