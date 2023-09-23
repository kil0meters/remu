// #![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use elf::{endian::AnyEndian, ElfBytes};
use emulator::Emulator;
use memory::Memory;

mod auxvec;
mod emulator;
mod instruction;
mod memory;
mod stack;
mod syscalls;

#[derive(Parser)]
struct Arguments {
    file: String,

    #[clap(short, long)]
    precache: bool,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

fn main() -> Result<()> {
    let args = Arguments::parse();

    env_logger::Builder::new()
        .format_timestamp(None)
        .target(env_logger::Target::Stdout)
        .filter_level(args.verbose.log_level_filter())
        .init();

    let file_data = std::fs::read(args.file).expect("Could not read file.");
    let slice = file_data.as_slice();
    let file = ElfBytes::<AnyEndian>::minimal_parse(slice)?;

    match (file.ehdr.class, file.ehdr.e_type, file.ehdr.e_machine) {
        // (64 bit, executable, risc_v arch)
        (elf::file::Class::ELF64, 0x02, 0xF3) => log::info!("Parsing executable."),
        got => {
            eprintln!(
                "Error. Invalid executable format. Expects a 64-bit RISC-V Linux binary. Got: {:x?}",
                got
            );
            return Ok(());
        }
    }

    let file_entry = file.ehdr.e_entry;
    let memory = Memory::load_elf(file);

    let mut emulator = Emulator::new(file_entry, memory);

    if args.precache {
        emulator.precache_instructions();
    }

    loop {
        if let Some(exit_code) = emulator.fetch_and_execute() {
            println!("------------------------------");
            println!("Program exited with code {exit_code}");
            println!("Fuel consumed: {}", emulator.fuel_counter);
            break;
        }
    }

    // emulator.print_registers();

    Ok(())
}
