use anyhow::Result;
use clap::Parser;
use elf::{endian::AnyEndian, ElfBytes};
use log::LevelFilter;
use simplelog::{ConfigBuilder, SimpleLogger};

use remu::{disassembler::Disassembler, emulator::Emulator, memory::Memory};

mod ui;

#[derive(Parser)]
struct Arguments {
    file: String,

    /// Path for a file to be treated as standard input
    #[clap(long)]
    stdin: Option<String>,

    /// Output the disassembly of the executable, then exit
    #[clap(short, long)]
    disassemble: bool,

    /// Enables an interactive reverse debugger
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

    if args.disassemble {
        println!("{}", Disassembler::disassemble_elf(&file));
        return Ok(());
    }

    let memory = Memory::load_elf(file);
    let mut emulator = Emulator::new(memory);

    if let Some(stdin_file) = args.stdin {
        let file_data = std::fs::read(stdin_file)
            .expect("Could not read file.")
            .leak();

        emulator.set_stdin(file_data);
    }

    if args.interactive {
        let mut app = ui::App::new(emulator)?;
        app.main_loop()
    } else {
        emulator.profile_label("main")?;

        loop {
            if let Some(exit_code) = emulator.fetch_and_execute()? {
                print!("{}", emulator.stdout);
                eprintln!("------------------------------");
                eprintln!("Program exited with code {exit_code}");
                eprintln!("Estimated cycle count: {}", emulator.profile_cycle_count);
                eprintln!(
                    "Cache hit/miss ratio: {}",
                    emulator.profile_cache_hit_count as f64
                        / emulator.profile_cache_miss_count as f64
                );
                eprintln!(
                    "Branch predict/misspredict ratio: {}",
                    emulator.profile_predicted_branch_count as f64
                        / emulator.profile_mispredicted_branch_count as f64
                );
                eprintln!(
                    "Estimated time to execute on a (bad) 4GHz processor: {}s",
                    emulator.profile_cycle_count as f64 / 4_000_000_000.0
                );
                break;
            }
        }

        Ok(())
    }
}
