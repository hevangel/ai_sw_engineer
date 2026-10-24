# micral_n runtime

Target-side code shared by all titles on this machine:

- reset vector / crt0 (stack setup, `.data` copy, `.bss` clear) — assembly for `llvm-i8008`
- target implementations of title HAL traits (keyboard scan, display, printer, ...)
- `#[panic_handler]` that halts or signals the simulator

Keep it minimal; everything portable belongs in the title's `lib.rs`.
