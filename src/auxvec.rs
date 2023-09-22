// source:
// https://android.googlesource.com/platform/bionic/+/android-7.1.1_r11/libc/kernel/uapi/linux/auxvec.h

#[derive(Debug, Clone, Copy)]
pub enum Auxv {
    Null = 0,
    Ignore = 1,
    ExecFd = 2,
    Phdr = 3,
    Phent = 4,
    Phnum = 5,
    Pagesz = 6,
    Baze = 7,
    Flags = 8,
    Entry = 9,
    NotElf = 10,
    Uid = 11,
    Euid = 12,
    Gid = 13,
    Egid = 14,
    Platform = 15,
    Hwcap = 16,
    Clktlk = 17,
    Secure = 23,
    BasePlatform = 24,
    Random = 25,
    HwCap2 = 26,
    Execfn = 31,
}

pub const RANDOM_BYTES: u64 = 16;
pub struct AuxPair(pub Auxv, pub u64);
