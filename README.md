# <img alt="REM" src="https://github.com/kil0meters/remu/assets/32966690/de8afc73-0599-4a4c-ba80-e354d688efb8" style="height: 64px"> U

**R**ISC-V **EMU**lator is a RV64GC/Linux emulator focused on providing advanced performance metrics.

Currently simulated CPU performance charactaristics:
- Memory Cache
    - One of the most problematic aspects of calculating performance based purely on the number of instructions executed is that loading data is very slow. REMU simulates a primitive CPU cache system:
        - Loading cached data requires 3 cycles for the data to propagate.
        - Loading non-cached data requires 200 cycles for the data to propagate.
- Pipeline Stalls
    - REMU simulates a fully-bypassed CPU pipeline, meaning there are no pipeline stalls for using data immediately after production. Example:
      ```asm
      ; this would typically generate some stall cycles depending on the CPU pipeline
      add a0, a1, a2
      lw  s0, 0(a0)
      ```
    - **HOWEVER:** Some instructions may still take multiple cycles to resolve their output. Using that data before it has resolved will cause a pipeline stall until the data is ready.
      - `MUL` family of instructions take 3 cycles
      - `DIV` family of instructions take a dynamic number of instructions depending on the ratio between the dividend and divisor.
      - As mentioned previously, loads take at least 3 cycles and up to 200 depending on whether the data was cached.
- Branch Predictor Misses
    - Because CPUs buffer multiple instructions in a pipeline, a branch would typically cause a pipeline stall. To alleviate this, CPUs will guess one side of the branch is executed and continue feeding the pathline along that path.
        - By default, REMU assumes all branches are not taken.
        - If a branch is taken, it assumes it will be taken next time.
        - Any mispredicted branch will incur a 4 cycle pipeline stall.
        - REMU has an unlimited number of branch predictor entries.

## puck

A command line front-end for Remu, featuring a disassembler and interactive reverse debugger.

```
Usage: puck [OPTIONS] <FILE>

Arguments:
  <FILE>

Options:
      --stdin <STDIN>  Path for a file to be treated as standard input
  -d, --disassemble    Output the disassembly of the executable, then exit
  -l, --label <LABEL>  The label to profile, default="main"
  -i, --interactive    Enables an interactive reverse debugger
  -v, --verbose...     More output per occurrence
  -q, --quiet...       Less output per occurrence
  -h, --help           Print help
```

![image](https://github.com/kil0meters/remu/assets/32966690/7618c807-2c85-4f1e-9496-e7606ab511b7)


### Building

**NOTE:** remu only supports little-endian CPU architectures.

Example usage on arch linux:
```
# pacman -S riscv64-linux-gnu-gcc
$ riscv64-linux-gnu-{gcc,g++} -O2 {your_program}
$ cargo run --release -- a.out
```
