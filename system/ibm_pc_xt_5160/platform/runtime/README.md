# ibm_pc_xt_5160 runtime

Target-side code shared by all titles on this machine:

- reset vector / crt0 (stack setup, `.data` copy, `.bss` clear) — assembly for `llvm-i8086`
- target implementations of title HAL traits (keyboard scan, display, printer, ...)
- `#[panic_handler]` that halts or signals the simulator

Keep it minimal; everything portable belongs in the title's `lib.rs`.

## Bring-up contract (design; code lands with the backend)

The crt0 establishes, per image kind (`spec/cpu_abi.md` §8):

| Image | Segments | Stack | Clears/copies |
|---|---|---|---|
| boot | `CS=DS=ES=SS=0` | `SP=7C00h` (below the sector) | `.bss` in low RAM; no `.data` copy (single segment) |
| com | `CS=DS=ES=SS` (from DOS) | keep DOS `SP` | `.bss` only |

- Direction flag: `CLD` first; `DF=0` across all calls (XT-A3).
- No interrupt handlers installed; no IVT writes; NMI stays as BIOS left it.
- `main` returns into a halt loop (`CLI; HLT`, spin) for bare metal, or
  `INT 21h/4Ch` for DOS-hosted images.
- Panic handler: bare metal writes `PANIC` to the CGA text page and halts;
  DOS-hosted exits with `AL=1` via `INT 21h/4Ch`.

The hand-written NASM baseline in `bringup/` approximates this contract for
the boot image only, to qualify the oracles before the Rust backend exists.
