# Busicom 141-PF platform

Memory map, I/O, linker script and the **image runner** that executes any 4004
ROM image on a whole-machine simulator and captures the printed tape.

- [memory_map.md](memory_map.md) — ROM/RAM spaces and the 1280-byte budget
  (plus the **Extended (xl) board** section: 16 × 4001 ROM + 16 × 4002 RAM)
- [io_map.md](io_map.md) — ports, keyboard matrix, printer hammer word
- [link.ld](link.ld) — target-image linker script (ROM 0x000–0x4FF, 4002 RAM
  arena + software-stack region; see the memory map below)
- [link_xl.ld](link_xl.ld) — **xl**-board linker script (ROM 0x000–0xFFF;
  `.data`/`.bss` on bank-0 chips 2–3, software stack on bank 1)
- [rtl/busicom_141pf_xl.sv](rtl/busicom_141pf_xl.sv) — the extended RTL board
  top, derived from the ai_hw_engineer `busicom_141pf.sv`
- `runtime/` — reset code and the port-level drivers (milestone M3);
  `runtime/tests/` includes the xl-board memory self-test
- `scripts/run_image.sh` — runs an image on the RTL board (primary) or
  `external/chips` (secondary) and emits the `rom_port_trace.txt` tape;
  `--board stock|xl` selects the hardware variant

## The image runner

```
scripts/run_image.sh <primary|secondary> <image.bin> <out_trace.txt> [stimulus]
scripts/compare.sh   <out_trace.txt> [oracle.txt]     # exact diff vs the oracle
```

`<image.bin>` is any 1280–4096-byte 4004 ROM image (the original mask, or a
future Rust build lowered by `llvm-i4004`). `[stimulus]` defaults to the
committed `firmware/oracle/manual_examples.json` — all 42 Unicom-141 manual
examples — converted to a deterministic key/switch schedule by
`scripts/build_schedule.py`. Both simulators are fed that **same** schedule, so
their tapes are directly comparable; a disagreement is a FINDING (AGENTS.md).

Output is EXACTLY the `firmware/oracle/rom_port_trace.txt` format: per example a
`#EXAMPLE id`, a `#RESET`/`#STEP n` marker per checkpoint, `LAMPS|n` (the full
4-bit RAM1 output port), and `ROW|value|symA|symB|red` per printed line.

### primary — ai_hw_engineer Busicom RTL board (Verilator)

The authoritative hardware model. `scripts/tb_trace_main.cpp` is a Verilator
C++ harness that Verilates the whole board from the **mounted, unmodified**
`ai_hw_engineer` RTL (`busicom_141pf.sv` + `intel_400{1,2,3,4}.sv`), loads the
image as the five 4001 masks (`scripts/split_rom.py` → `src/rom/rom_4001_N.hex`,
read by the board's `$readmemh` against the run CWD), and drives the front-panel
interface exactly as `ai_hw_engineer/tb/tb_top.sv` does:

- one machine cycle = 8 full clock periods;
- drum/TEST/index pacing: `BUSICOM_SPIN` machine cycles per drum half-spin
  (default **1481**, the reference value the oracle was captured at), TEST high
  on even half-spins, index pulse + drum reset at half-spin 26;
- keys/switches presented via `keys_mask_i`/`precision_i`/`rounding_i` (the
  board does the keyboard-matrix decode and the 20-bit 4003 printer shift chain
  in RTL);
- events polled once per half-spin — the reference DPI-bridge cadence — reading
  the one-shot `hammer_evt`/`hammer_data` (`{drum_pos, 20-bit word}`),
  `advance_evt`, `red`, and the **4-bit** RAM1 port (`--public-flat-rw`; see the
  lamp note below), then acking with a `panel_tick` toggle so the board re-arms.

Needs Verilator — present in `ai-hw-engineer:latest`, **absent** from
`ai-sw-engineer:latest`. Run the primary path in the hw image.

### secondary — external/chips (V. Ilmer MCS-4 board model)

An independent third-party oracle. `scripts/chips_trace.rs` is a path-dependency
adapter on the **pinned, unmodified** `external/chips`
(`1bc3d5781474ef9d7e7305af91071a723797f222`) — a generalisation of
`ai_hw_engineer/scripts/reference_oracle.rs` whose only change is that the ROM
image is an argument, so the same board wiring and drum decode that captured the
committed oracle is reused to score our own images. The crate is run, never
copied (no license). `git submodule update --init external/chips` first, or
point `CHIPS_DIR` at a pinned checkout.

## Verification (original ROM)

Both simulators run the **original 1280-byte mask**
(`ai_hw_engineer/.../rom_141pf_combined.bin`, SHA-256 in
`firmware/original/provenance.md`; binary gitignored) and are diffed against
`firmware/oracle/rom_port_trace.txt`:

| Simulator | Result | Runtime (42 examples) |
|---|---|---|
| **primary** (RTL, Verilator, spin=1481) | **846/846 payload lines identical, 42/42** | ~34 min (~49 s/example) |
| **secondary** (chips) | **846/846 payload lines identical, 42/42** | ~11 s (~0.27 s/example) |

The RTL also prints **identical ROWs** down to `spin=740` in ~8 min; below ~740
the firmware's per-sector print burst no longer completes within a half-spin.
The two simulators agree with each other (both equal the oracle) — **no RTL-vs-
chips disagreement**. Every printed value, symbol column, red flag and row order
matches on both; see "lamp bit 3" below for the one port-width caveat.

Reproduce (inside `ai-hw-engineer:latest`, both repos mounted):

```bash
# secondary (fast)
CHIPS_DIR=/path/to/external/chips \
  scripts/run_image.sh secondary rom_141pf_combined.bin /tmp/t_chips.txt
scripts/compare.sh /tmp/t_chips.txt         # PASS 42/42

# primary (RTL, slow)
AI_HW_ENGINEER=/ai_hw_engineer BUSICOM_SPIN=1481 \
  scripts/run_image.sh primary rom_141pf_combined.bin /tmp/t_rtl.txt
scripts/compare.sh /tmp/t_rtl.txt           # PASS 42/42
```

## Extended (xl) board variant

The compiled Rust firmware does not fit the stock board (runtime+crt0+HAL ≈ 697
of 1280 ROM bytes, and the firmware's static state exceeds the stock 64-byte
main RAM). The **xl** board keeps every Busicom I/O role identical and only adds
memory, to the architectural maximum: **16 × 4001** ROM (4096 B) and **16 ×
4002** RAM (640 B, 4 CM-RAM banks). See `memory_map.md` "Extended (xl) board"
for the chip complement and map, `link_xl.ld` for the regions, and
`rtl/busicom_141pf_xl.sv` for the RTL top (derived from the ai_hw_engineer
`busicom_141pf.sv`, instantiating the same `intel_400x` modules by path; the
front-panel capture and ROM-port select translator are copied verbatim).

Select it with `run_image.sh --board xl` (default `stock`). The same runner,
harness and chips adapter serve both boards:

- **primary (RTL):** splits the image into 16 masks, Verilates the `*_xl` top
  via a generated `board_sel.h` (so `tb_trace_main.cpp` compiles for either
  board with no `-D` quote mangling), reads the 4-bit RAM1 lamp port from the
  xl top's `ram1_port4` net.
- **secondary (chips):** runs with `ram_count=4`. **FINDING:** the unmodified
  external `chips` model stores a 4002 identity in a 2-bit `page_mask` and
  `Board::new` panics for a chip index ≥ 4, so it can model only **one** CM-RAM
  bank (≤ 4 chips). Bank 0 (4 chips) is the common ground both simulators run;
  banks 1–3 are validated on the RTL board only. We do not edit `external/`.

### xl verification

Original 1280-byte ROM on the xl board (extra chips are invisible to it — it
never fetches past 0x4FF or selects a RAM chip it was not wired to use):

| Simulator | Result |
|---|---|
| secondary (chips, ram_count=4) | **846/846 payload lines identical, 42/42** |
| primary (RTL, spin=1481) | **42/42** (full run) · 2/2 head confirmed at spin=760 |

Added-memory self-test (`runtime/tests/`, verdict on the RAM1 lamp port,
`LAMPS|7` = PASS):

| Test | Simulator | Verdict |
|---|---|---|
| bank-0 256 cells (4 × 4002, incl. added chips 2,3) | chips | **PASS** |
| bank-0 256 cells | RTL | **PASS** |
| full 4-bank 1024 cells (all 16 × 4002, DCL per bank) | RTL | **PASS** |

Negative control: the same bank-0 test with only 2 RAM chips present (added
chips 2,3 absent) reports `LAMPS|1` = FAIL — the probe genuinely distinguishes
present from absent memory. Build/run details: `runtime/tests/README_memtest_xl.md`.



**Question (from the llvm-i4004 agent):** `external/chips` jumps on a `JCN` with
condition bit C4 set when its internal TEST flag is HIGH; the MCS-4 manual says
jump when TEST = 0. Which is right, and what does the Busicom board drive on
TEST?

**Answer: the MCS-4 manual is right — a JCN C4 jump is taken when the TEST pin
is LOW.** The `chips` decoder is inverted relative to the manual; the committed
oracle is nonetheless correct because the capture adapter compensates by driving
`chips` the inverse value, and the ai_hw_engineer RTL implements the manual
polarity directly. Evidence:

1. **ai_hw_engineer 4004 RTL — manual polarity, jump on TEST LOW.**
   `ai_hw_engineer/design/intel_4004/src/intel_4004.sv`:
   ```systemverilog
   // JCN condition (spec 5.4). C1 = invert, C2 = ACC zero, C3 = CY, C4 = TEST
   assign jcn_raw = ((opa[2] && (acc == 4'h0)) ||
                     (opa[1] && cy) ||
                     (opa[0] && !test_i));          // C4 true when TEST is LOW
   assign jcn_taken = opa[3] ? !jcn_raw : jcn_raw;
   ```
   and the port comment: `// Historical TEST pin (JCN condition code C4 samples
   it low)`. This matches the Intel MCS-4 manual (JCN condition 4: "jump if TEST
   = 0").

2. **external/chips — inverted, jump on TEST HIGH.**
   `external/chips/src/mcs4/cpu4004.rs`, `ContinueFrom::JumpConditional`:
   ```rust
   let c4 = matches!(modifier & 0x1, 0x1); //Test
   let partial_cond = c4 & self.test || c3 & self.carry || c2 & (self.acc == u4::new(0));
   ```
   i.e. the condition is satisfied when `self.test` is **true (HIGH)** — the
   opposite of the manual.

3. **Why the oracle still matches.** The capture adapter
   (`reference_oracle.rs`, and our `chips_trace.rs`) drives `chips` with
   `cpu.set_test_flag(spin % 2 == 0)` — the **logical inverse** of the pin level
   the board presents — so `chips`'s inverted decode lands on the same branch the
   RTL takes from the true pin level. The two models therefore agree bit-for-bit
   on the 42-example tape (proven above), despite the opposite internal
   convention.

4. **What the Busicom board drives on TEST.** The printer-drum sector sync: the
   board/testbench toggles TEST every drum half-spin (`io_map.md`;
   `ai_hw_engineer` spec §3.3 "the CPU `TEST` input … toggles every drum
   half-spin", §4.2; `tb_top.sv` sets `test_i` high on even half-spins). The
   firmware busy-waits on TEST (`JCN`/`JNT`) to pace printing. It is an active
   **level** sampled by JCN, with the manual's "jump when LOW" semantics.

**Recommendation for `llvm-i4004/spec/isa.md` (do NOT edit from this task —
noted for the backend agent):** state JCN condition bit C4 (the `0x1` bit of the
condition nibble) as **"jump when the TEST pin is LOW (logic 0)"**, matching the
Intel MCS-4 User's Manual and the ai_hw_engineer RTL. If the ISA doc or any
backend test derives expected behaviour by running `external/chips` directly,
add a note that `chips` uses the inverted internal convention and must be driven
with the pin's logical inverse (as the oracle capture does) — otherwise a
backend `JCN` test validated against bare `chips` would encode the wrong
polarity.

## Lamp bit 3 (RAM1), and what is unverified

- **Lamp "bit 3".** The oracle's `LAMPS|n` is the full 4-bit RAM1 output port
  (`chips`: `rams[1].ports.value()`). The ROM sets an **undocumented RAM1 bit 3**
  on overflow (nibble 10 = 0b1010) and on negative totals (nibble 12 = 0b1100) —
  `firmware/spec/spec.md` assumption **A9**. The RTL board's `lamps_o` port is
  only **3 bits** (memory / overflow / negative), so bit 3 is not visible there;
  this harness reads the internal 4-bit `ram1_port` (Verilator
  `--public-flat-rw`) to capture it, and then the RTL `LAMPS` values match the
  oracle exactly (12 and 10, not the 3-bit-truncated 4 and 2). The three *named*
  lamps a user sees (memory/overflow/negative) are bits 0/1/2; bit 3 drives no
  documented front-panel lamp.
- **Oracle scope.** Both proofs use the recovered-ROM profile of the 42 manual
  examples (135 checkpoints). Behaviour outside those inputs (other key
  sequences, error-recovery paths) is not exercised here.
- **RTL timing.** Rows are proven identical at `spin=1481` and `spin=740`.
  Sub-sector analog hammer timing is not modelled (a documented non-goal; the
  reconstructed `{value,symbol,red}` row is identical either way).
- **Pending the compiled image.** The runner is proven with the ORIGINAL ROM.
  It will score a Rust-built image byte-for-byte the same way once `llvm-i4004`
  can lower the firmware (instruction selection in progress on another branch);
  nothing in this task depends on that image existing.
