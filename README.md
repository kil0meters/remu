# <img src="https://github.com/kil0meters/remu/assets/32966690/de8afc73-0599-4a4c-ba80-e354d688efb8" style="height: 64px"> U

A RV64C/Linux Emulator focused on providing advanced performance metrics

### Usage:

Only tested on Arch x86\_64:

```
# pacman -S riscv64-linux-gnu-gcc

$ riscv64-linux-gnu-{gcc,g++} -O2 {your_program}

$ cargo run --release -- a.out
```

### Example:

```c
#include <stdio.h>
int main() {
    printf("hello world");
}
```

```
$ cargo run -- a.out
dynamic links with: libc.so.6
hello world
------------------------------
Program exited with code 0
Fuel consumed: 105891
```
