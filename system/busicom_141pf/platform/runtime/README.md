# busicom_141pf runtime

Target-side code for this machine: the board HAL glue the Rust firmware calls to
reach the MCS-4 bus, plus a hand-written smoke program that drives the board
through that glue. The chip-independent C-runtime startup (`crt0.s`) and the
arithmetic/memory runtime live in `llvm-i4004/rt/` (shared by every i4004
title); only the board-specific glue is here.

## HAL glue — `hal.s`

Four C-ABI functions (abi.md §4). **This is a fixed interface**: the Rust target
HAL (`firmware/src/main.rs` via the platform HAL trait) codes against exactly
these symbols and signatures, so renaming one or reordering arguments breaks the
firmware build. Each is a leaf that clobbers only ACC/CY/P0; the caller's
callee-saved `P4–P6`/SSP survive.

| Symbol | Signature | Bus op | Size (B) |
|---|---|---|---|
| `__bus_rom_port_write` | `void(u8 chip=P1, u8 value=P2)` | `SRC{chip in [7:4]}; WRR` | 8 |
| `__bus_rom_port_read` | `u8(u8 chip=P1)` → P1 | `SRC{chip in [7:4]}; RDR` | 10 |
| `__bus_ram_port_write` | `void(u8 chip=P1, u8 value=P2)` | `DCL 0; SRC{chip in [7:6]}; WMP` | 14 |
| `__bus_test_pin` | `u8(void)` → P1 (1 if HIGH else 0) | `JCN` on cond C4 | 11 |

Chip-select detail (isa.md §5, io_map.md):

- A **ROM I/O port** (4001, `WRR`/`RDR`) is selected by `SRC[7:4]` = the ROM
  chip index (the full high nibble). `SRC[3:0]` is ignored by the port.
- A **RAM output port** (4002, `WMP`) is selected by `SRC[7:6]` = the 4002 chip
  index within the DCL-selected bank — so the index is placed in the **top two
  bits** of the SRC byte, i.e. `(chip << 2)` in the high nibble. `RAM0` (the
  print/paper lamps) is chip 0 → high nibble `0x0`; `RAM1` (memory / overflow /
  negative lamps) is chip 1 → high nibble `0x4`. The Busicom board uses CM-RAM
  bank 0.
- The **TEST pin** is sampled by `JCN` condition bit C4, whose jump is taken
  when TEST is **LOW** (platform/README.md "JCN TEST-pin polarity"; MCS-4 manual
  and the ai_hw_engineer RTL agree). `__bus_test_pin` returns 1 for HIGH, 0 for
  LOW. `external/chips` uses the inverse internal convention; the unit test
  (below) accounts for that, and the result matches the RTL.

### i8 return caveat

`BBL` reloads ACC on return (isa.md §1), so `__bus_rom_port_read`/`__bus_test_pin`
return their `u8` in **P1** (low nibble = value, high nibble zeroed), per the
runtime ABI refinement documented in `llvm-i4004/rt/README.md`.

## Smoke program — `smoke.s` + `build_smoke.sh`

A hand-written program linked with `crt0.s` + `hal.s` that drives the board
through the glue: it reads the TEST pin, lights the RAM1 lamp port with the
fixed signature `0b0111 = 7` (a value the original firmware never leaves set),
and clears the RAM0 output port, forever. It proves the glue path end to end
without the Rust firmware or the codegen backend.

```
docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
  ai-sw-engineer:latest bash system/busicom_141pf/platform/runtime/build_smoke.sh out.bin
# then run it on either simulator via scripts/run_image.sh and grep LAMPS:
scripts/run_image.sh secondary out.bin trace.txt      # chips
scripts/run_image.sh primary   out.bin trace.txt sched # RTL (Verilator)
```

**Verified:** `LAMPS|7` on **both** simulators — chips (secondary, all 162
sampled steps) and the ai_hw_engineer RTL board (primary, Verilator) — with no
spurious printed rows. The two models agree, so the glue drives the real
hardware model, not just the emulator.

## HAL unit tests — `tests/`

`tests/run_hal_tests.sh` builds a bare-CPU `chips` harness (`tests/hal_test.rs`)
that can drive the TEST pin to a chosen level and read back any RAM/ROM port or
RAM character, then exercises each glue function with a hand-written stub and
checks it against a Rust oracle. 10/10 pass, including `__bus_test_pin` at both
pin levels and a ROM-port write→read round trip.

```
docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
  ai-sw-engineer:latest bash system/busicom_141pf/platform/runtime/tests/run_hal_tests.sh
```

A `#[panic_handler]` for the Rust firmware and the keyboard/printer HAL trait
implementations are added when the firmware is lowered by the backend (in
progress on another branch); this runtime provides the bus primitives they build
on.
