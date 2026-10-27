# ibm_pc runtime

Target-side code shared by all titles on this machine:

- reset vector / crt0 (stack setup, `.data` copy, `.bss` clear) — assembly for `llvm-i8086`
- target implementations of title HAL traits (keyboard scan, display, printer, ...)
- `#[panic_handler]` that halts or signals the simulator

Keep it minimal; everything portable belongs in the title's `lib.rs`.

## DOS programs (.COM)

Titles that run under PC DOS use a second start-up and linker script instead of the boot-sector pair:

| File | Role |
|---|---|
| `crt0_com.s` | entry at CS:0100h: SP from the top of memory in the PSP (word at 0002h, at most one segment), clears `.bss`, calls `main`, ends with INT 20h |
| `dos.s` | `dos_call(&Regs)`: loads AX..ES/DS from a register block, executes INT 21h, stores the registers and flags back; `dos_exit` (INT 20h) |
| `../link_com.ld` | one region at 0100h; `i8086-ld --com x.com` writes the `.COM`, and `--exe x.exe` the same image behind an MZ header (CS = SS = the PSP segment, so `crt0_com.s` starts it unchanged) |
| `../dos/` | Rust crate `ibm_pc_dos`: `Regs`, `int21`, `putc`, `println!`, `exit` |

The first program is `llvm-i8086/tests/smoke/hello_com`; `emulator/tests/i8086_dos.rs` runs it under the real PC DOS 1.00 and compares the screen with the RTL's (`emulator/oracle/dos/hello_com.txt`).
