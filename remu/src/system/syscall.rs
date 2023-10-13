// initially taken from this highly inaccurate blog post:
// https://jborza.com/post/2021-05-11-riscv-linux-syscalls/
// then some edits made for correctness from linux kernel source code

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{error::RVError, files::*, register::*, system::FileDescriptor};

use super::Emulator;

#[derive(FromPrimitive, Debug)]
pub enum Syscall {
    Ioctl = 29,
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

impl Emulator {
    // emulates linux syscalls
    pub(super) fn syscall(&mut self) -> Result<(), RVError> {
        let id = self.x[A7];
        let arg = self.x[A0];

        let sc: Syscall = FromPrimitive::from_u64(id).expect(&format!(
            "{:16x} {} Unknown syscall: {id}",
            self.pc, self.inst_counter
        ));

        // log::info!("{:x}: executing syscall {sc:?}", self.pc);

        match sc {
            Syscall::Ioctl => {
                self.x[A0] = 0;
            }

            Syscall::Faccessat => {
                self.x[A0] = -1i64 as u64;
                // TODO: currently just noop (maybe that's fine, who knows)
            }

            Syscall::Openat => {
                let fd = self.x[A0] as i64;
                let filename = self.memory.read_string_n(self.x[A1], 512)?;
                let _flags = self.x[A1];

                log::info!("Opening file fd={fd}, name={filename}");
                // log::info!("Flags={_flags:b}");

                if filename == "/lib/tls/libc.so.6" {
                    self.file_descriptors.insert(
                        LIBC_FILE_DESCRIPTOR,
                        FileDescriptor {
                            offset: 0,
                            data: LIBC_DATA.into(),
                        },
                    );

                    self.x[A0] = LIBC_FILE_DESCRIPTOR as u64;
                } else if filename == "/lib/tls/libstdc++.so.6" {
                    self.file_descriptors.insert(
                        LIBCPP_FILE_DESCRIPTOR,
                        FileDescriptor {
                            offset: 0,
                            data: LIBCPP_DATA.into(),
                        },
                    );

                    self.x[A0] = LIBCPP_FILE_DESCRIPTOR as u64;
                } else if filename == "/lib/tls/libm.so.6" {
                    self.file_descriptors.insert(
                        LIBM_FILE_DESCRIPTOR,
                        FileDescriptor {
                            offset: 0,
                            data: LIBM_DATA.into(),
                        },
                    );

                    self.x[A0] = LIBM_FILE_DESCRIPTOR as u64;
                } else if filename == "/lib/tls/libgcc_s.so.1" {
                    self.file_descriptors.insert(
                        LIBGCCS_FILE_DESCRIPTOR,
                        FileDescriptor {
                            offset: 0,
                            data: LIBGCCS_DATA.into(),
                        },
                    );

                    self.x[A0] = LIBGCCS_FILE_DESCRIPTOR as u64;
                } else {
                    self.x[A0] = (-1i64) as u64;
                }
            }

            Syscall::Close => {
                let fd = self.x[A0] as i64;

                if self.file_descriptors.remove(&fd).is_some() {
                    self.x[A0] = 0;
                } else {
                    self.x[A0] = -1i64 as u64;
                }
            }

            Syscall::Lseek => {
                let fd = self.x[A0] as i64;
                let offset = self.x[A1];
                let whence = self.x[A2];

                match self.file_descriptors.get_mut(&fd) {
                    Some(descriptor) => {
                        match whence {
                            // SEEK_SET
                            0 => {
                                descriptor.offset = offset;
                            }

                            // SEEK_CUR
                            1 => {
                                descriptor.offset = descriptor.offset.wrapping_add(offset);
                            }

                            // SEEK_END
                            2 => {
                                descriptor.offset =
                                    (descriptor.data.len() as u64).wrapping_add(offset);
                            }

                            _ => {
                                self.x[A0] = -1i64 as u64;
                            }
                        }
                    }
                    None => {
                        self.x[A0] = -1i64 as u64;
                    }
                }
            }

            Syscall::Read => {
                let fd = self.x[A0] as i64;
                let buf = self.x[A1];
                let count = self.x[A2];

                log::info!("Reading {count} bytes from file fd={fd} to addr={buf:x}");

                if let Some(entry) = self.file_descriptors.get_mut(&fd) {
                    self.x[A0] = self.memory.read_file(entry.into(), buf, count)? as u64;
                } else {
                    self.x[A0] = -1i64 as u64;
                }
            }

            Syscall::Write => {
                let fd = self.x[A0];
                assert!(fd <= 2);

                let ptr = self.x[A1];
                let len = self.x[A2];

                log::info!(
                    "Writing to file={}, addr={:x}, nbytes={}",
                    self.x[A0],
                    self.x[A1],
                    self.x[A2]
                );

                let s = self.memory.read_string_n(ptr, len)?;
                self.stdout.push_str(&s);

                self.x[A0] = len;
            }

            Syscall::Writev => {
                let fd = self.x[A0];
                assert!(fd <= 2);

                let iovecs = self.x[A1];
                let iovcnt = self.x[A2];

                for i in 0..iovcnt {
                    let ptr = self.memory.load(iovecs + (i * 16))?;
                    let len = self.memory.load(iovecs + 8 + (i * 16))?;

                    let s = self.memory.read_string_n(ptr, len)?;
                    self.stdout.push_str(&s);
                }
            }

            Syscall::Readlinkat => {
                // let dirfd = self.x[A0];
                let addr = self.x[A1];
                let buf_addr = self.x[A2];
                let bufsize = self.x[A3];

                let s = self.memory.read_string_n(addr, 512)?;

                if s == "/proc/self/exe" {
                    self.memory.write_n(b"/prog\0", buf_addr, bufsize)?;
                    self.x[A0] = 5;
                } else {
                    self.x[A0] = -1i64 as u64;
                }
            }

            Syscall::Exit => {
                log::info!("Exiting with code {arg}");
                self.exit_code = Some(arg);
            }

            Syscall::ExitGroup => {
                log::info!("Exiting with code {arg}");
                self.exit_code = Some(arg);
            }

            Syscall::SetTidAddress => {
                self.x[A0] = 0;
            }

            Syscall::Futex => {
                let uaddr = self.x[A0];
                let futex_op = self.x[A1];
                let _val = self.x[A2];
                let _timeout_addr = self.x[A3];
                let _val3 = self.x[A4];

                // FUTEX_WAIT
                if futex_op == 128 {
                    self.memory.store(uaddr, 0u64)?;
                }

                self.x[A0] = 0;
            }

            Syscall::SetRobustList => {
                self.x[A0] = 0;
            }

            Syscall::ClockGettime => {
                // noop
            }

            Syscall::Tgkill => {
                self.x[A0] = -1i64 as u64;
            }

            Syscall::RtSigaction => {
                self.x[A0] = 0;
            }

            Syscall::RtSigprocmask => {
                self.x[A0] = 0;
            }

            Syscall::Getpid => {
                self.x[A0] = 0;
            }

            Syscall::Gettid => {
                self.x[A0] = 0;
            }

            Syscall::Brk => {
                let addr_before = self.memory.brk(0);
                self.x[A0] = self.memory.brk(arg);

                log::info!(
                    "Allocated {} bytes of memory to addr=0x{addr_before:x}",
                    self.x[A0] - addr_before
                );
            }

            Syscall::Munmap => {
                // who needs to free memory
                self.x[A0] = 0;
            }

            Syscall::Mmap => {
                let addr = self.x[A0];
                let len = self.x[A1];
                let _prot = self.x[A2];
                let flags = self.x[A3];
                let fd = self.x[A4] as i64;
                let offset = self.x[A5];

                log::info!(
                    "mmap: Allocating {len} bytes fd={}, offset={offset} requested addr={addr:x} flags={flags}",
                    fd as i64
                );

                if fd == -1 {
                    // Only give address if MMAP_FIXED
                    if (flags & 0x10) != 0 {
                        self.x[A0] = self.memory.mmap(addr, len) as u64;
                    } else {
                        self.x[A0] = self.memory.mmap(0, len) as u64;
                    }
                } else if let Some(descriptor) = self.file_descriptors.get_mut(&fd) {
                    self.x[A0] = self.memory.mmap_file(descriptor, addr, offset, len)? as u64;
                } else {
                    self.x[A0] = -1i64 as u64;
                }
            }

            Syscall::Mprotect => {
                self.x[A0] = 0;
            }

            Syscall::Prlimit64 => {
                self.x[A0] = 0;
            }

            Syscall::Getrandom => {
                let buf = self.x[A0];
                let buflen = self.x[A1];

                // we want this emulator to be deterministic
                for i in buf..(buf + buflen) {
                    self.memory.store::<u8>(i, 0xff)?;
                }

                self.x[A0] = buflen;
            }
            Syscall::Newfstatat => {
                let fd = self.x[A0] as i64;
                let pathname_ptr = self.x[A1];
                let _statbuf = self.x[A2];
                let flags = self.x[A3];

                let pathname = self.memory.read_string_n(pathname_ptr, 512)?;
                log::info!("newfstatat for fd={fd} path=\"{pathname}\" flags={flags}");

                if fd == -1 {
                    self.x[A0] = 0;
                } else {
                    self.x[A0] = 0;
                }
            }
            Syscall::SchedYield => {
                self.x[A0] = 0;
            }
        }

        Ok(())
    }
}
