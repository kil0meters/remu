pub const LD_LINUX_DATA: &'static [u8] = include_bytes!("../../res/ld-linux-riscv64-lp64d.so.1");
pub const LIBC_DATA: &'static [u8] = include_bytes!("../../res/libc.so.6");
pub const LIBCPP_DATA: &'static [u8] = include_bytes!("../../res/libstdc++.so");
pub const LIBM_DATA: &'static [u8] = include_bytes!("../../res/libm.so.6");
pub const LIBGCCS_DATA: &'static [u8] = include_bytes!("../../res/libgcc_s.so.1");

pub const LIBC_FILE_DESCRIPTOR: i64 = 10;
pub const LIBCPP_FILE_DESCRIPTOR: i64 = 11;
pub const LIBM_FILE_DESCRIPTOR: i64 = 12;
pub const LIBGCCS_FILE_DESCRIPTOR: i64 = 13;

#[derive(Clone)]
pub struct FileDescriptor {
    // current file read location
    pub offset: u64,
    pub data: Box<[u8]>,
}
