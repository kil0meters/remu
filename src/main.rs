// #![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use elf::{endian::AnyEndian, ElfBytes};
use emulator::{Emulator, InstCache};
use log::LevelFilter;
use memory::Memory;
use simplelog::{ConfigBuilder, SimpleLogger};

mod auxvec;
mod disassembler;
mod emulator;
mod instruction;
mod memory;
mod register;
mod syscalls;
mod time_travel;
mod ui;

#[derive(Parser)]
struct Arguments {
    file: String,

    #[clap(short, long)]
    cache: bool,

    #[clap(short, long)]
    interactive: bool,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

fn main() -> Result<()> {
    let args = Arguments::parse();
    let config = ConfigBuilder::new()
        .set_time_level(LevelFilter::Trace)
        .set_thread_level(LevelFilter::Trace)
        .build();

    SimpleLogger::init(args.verbose.log_level_filter(), config)?;

    let file_data = std::fs::read(args.file).expect("Could not read file.");
    let slice = file_data.as_slice();
    let file = ElfBytes::<AnyEndian>::minimal_parse(slice)?;

    match (file.ehdr.class, file.ehdr.e_type, file.ehdr.e_machine) {
        // (64 bit, executable, risc_v arch)
        (elf::file::Class::ELF64, 0x03 | 0x02, 0xF3) => log::info!("Parsing executable."),
        got => {
            eprintln!(
                "Error. Invalid executable format. Expects a 64-bit RISC-V Linux binary. Got: {:x?}",
                got
            );
            return Ok(());
        }
    }

    let memory = Memory::load_elf(file, args.interactive);
    let mut emulator = Emulator::new(memory);

    if args.interactive {
        ui::main_loop(emulator)
    } else {
        let mut inst_cache = args.cache.then(InstCache::default);

        loop {
            if let Some(exit_code) = emulator.fetch_and_execute(inst_cache.as_mut()) {
                print!("{}", emulator.stdout);
                println!("------------------------------");
                println!("Program exited with code {exit_code}");
                println!("Fuel consumed: {}", emulator.inst_counter);
                println!("Peak memory usage: {} bytes", emulator.max_memory);
                break;
            }
        }

        Ok(())
    }
}
