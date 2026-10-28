# atari_400_800 runtime

Target-side code shared by all titles on this machine:

- reset vector / crt0 (stack setup, `.data` copy, `.bss` clear) — assembly for `llvm-mos6502`
- target implementations of title HAL traits (keyboard scan, display, printer, ...)
- `#[panic_handler]` that halts or signals the simulator

Keep it minimal; everything portable belongs in the title's `lib.rs`.
