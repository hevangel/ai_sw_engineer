# Busicom 141-PF browser frontend — specification and UI/backend contract

Owning issue: hevangel/ai_sw_engineer#83 (Mode A delivered here; Mode B is a
contract with an explicit unavailable state). The execution engine of Mode A is
the qualified native Rust system emulator
([../emulator](../emulator/README.md)) compiled to WebAssembly — the same
`Emulator` core that replays the original firmware 42/42 against the committed
original-ROM trace. No second, browser-only machine model exists.

## Machine profile

| Aspect | Value | Source |
|---|---|---|
| CPU | Intel 4004 (MCS-4), ~740 kHz, 8 clocks/cycle | [../README.md](../README.md) |
| ROM | 5 × 4001 = 1,280 B original firmware; flat images up to 4,096 B supported (compiled xl-board firmware) | emulator `Emulator::new` (1..=4096 B, padded to 256-B pages) |
| RAM | 2 × 4002 (-1) | [../README.md](../README.md) |
| I/O | 3 × 4003 shift registers, keyboard matrix, drum printer, 3 lamps, 2 selectors | [../platform/io_map.md](../platform/io_map.md) |

Selectors: decimal point `0 1 2 3 4 5 6 8` (8 = eight decimals; there is no 7
position), rounding `0` FL / `1` 5/4 / `8` IN. The keyboard is 4 matrix columns
× 8 populated rows, codes `129 + 4·row + column` ([io_map](../platform/io_map.md)).

## Mode A — default Rust/WebAssembly machine

### Execution model

- Everything runs in the user's browser from static files: no server runtime,
  no remote simulator, no native install (AGENTS.md: the site stays static).
- The WASM module and the emulator run inside a Web worker, so UI input stays
  responsive while the machine executes.
- **Bounded batches**: the browser never runs an unbounded loop. One message =
  one machine event; the largest possible event is the 2,000-half-spin power-on
  settle ≈ 2,000 × 1,481 ≈ 2.96 M chip cycles, well under a second in the
  release WASM build. Key presses use the same 400 half-spins as the
  deterministic schedule ([build_schedule.py](../platform/scripts/build_schedule.py)).
- **Determinism**: given the same image and event sequence, the browser machine
  produces the same rows and lamps as the native emulator and CLI. Reset
  (`#`-power-cycle) restores the pristine state; there is no cross-session
  persistence and no snapshot format yet.

### WASM ABI contract

The binding crate is `web/wasm` (`busicom-emulator-wasm`, cdylib, no external
dependencies — raw C ABI, no wasm-bindgen). All functions are `extern "C"`;
`usize` is wasm32 pointer width.

| Export | Signature | Contract |
|---|---|---|
| `busicom_rom_scratch` | `(len: usize) -> *mut u8` | Ensures a stable scratch buffer of `len` bytes and returns its base. The host writes the image bytes there. |
| `busicom_init` | `(len: usize) -> u32` | Builds the machine from the scratch bytes. `0` = ok; `u32::MAX` = error, message in the output buffer. |
| `busicom_reset` | `() -> u32` | Power-on reset (always `0`). |
| `busicom_lamps` | `() -> u32` | RAM1 port nibble: bit 0 memory, bit 1 overflow, bit 2 negative. |
| `busicom_advance` | `(precision: u8, rounding: u8, key: u8, half_spins: u32) -> u32` | One key/switch event. Returns the byte length of the rows written to the output buffer (`ROW\|…` lines joined with `\n`), or `u32::MAX` on invalid input. |
| `busicom_out_ptr` / `busicom_out_len` | `() -> (*const u8, usize)` | Output buffer access; the host re-reads `memory.buffer` after every call (the buffer may have grown). |
| `busicom_build_id` | `() -> *const u8` | NUL-terminated build identifier for the about line. |

Memory ownership: the module owns all buffers; the JS host only reads linear
memory and never retains views across calls.

### UI contract

- **Printer tape**: each `ROW|value|symA|symB|red` line appends one paper row.
  `value` renders right-aligned monospace; `red=true` renders red (negative
  ink); a 15-dot `value` is the overflow line (dotted rule, no value); `symA` /
  `symB` render as the drum's symbol columns (e.g. `◇`, `*`, `M`, `CM`).
- **Lamps**: three LEDs — MEMORY, OVERFLOW, NEGATIVE — from
  `busicom_lamps()` bits 0/1/2, refreshed after every event (per A9, OVERFLOW
  and NEGATIVE indicate `overflow ∥ negative`).
- **Keypad**: the 30 populated keys in the physical layout
  ([io_map](../platform/io_map.md)); every key is also reachable from the PC
  keyboard (mapping shown in the UI). Selectors are dropdowns with exactly the
  legal positions.
- **ROM loading**: file input only. The page never bundles, fetches or commits
  firmware. On load it shows size and SHA-256 (`crypto.subtle`) and labels the
  image `original Busicom firmware` only when the hash equals the recorded
  provenance hash (`c6af325d…ceb7`, [../firmware/original/provenance.md](../firmware/original/provenance.md));
  any other image is `user-provided image`. Non-redistributable firmware stays
  out of the repository.
- **Hosting**: `www/` is plain HTML/CSS/ES modules; any static file server
  works (`py -m http.server` documented in the README). `file://` does not
  work (worker + fetch). The built `busicom_emulator_wasm.wasm` is copied into
  `www/` by [scripts/build.sh](../scripts/build.sh) and is gitignored.

### Known limitations (honest)

- The paper-advance button (ROM2 bit 3) is not exposed by the emulator API, so
  the UI has no paper-advance control.
- No audio (the machine has none), no front-panel debug access beyond the
  printer/lamps, no state snapshot.

## Mode B — optional Xezim/SystemVerilog machine (contract, not delivered)

The backend selector offers `SystemVerilog (Xezim)` next to the default. Selecting it shows an
explanatory **unavailable** state, never a broken default and never a Rust model
presented as SV simulation. Mode B requires, per issue #83:

- the qualified Xezim adapter and supported transport (ai_hw_engineer#69),
- the shared HDL Workbench/public-system adapter integration (ai_engineer#55),
- Surfer waveform integration or a qualified viewer (ai_hw_engineer#68),
- a normalized input/output and trace-time contract shared with Mode A.

Until those land, the SV option states what is missing and what it would
provide (real timestamped SV signals, waveform capture, slower cycle-true
execution). The default Rust/WASM machine never depends on any of it.
