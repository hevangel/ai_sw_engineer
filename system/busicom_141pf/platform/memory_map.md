# Busicom 141-PF memory map

MCS-4 has separate address spaces; there is no unified memory map.

| Space | Range | Size | Devices | Use |
|---|---|---|---|---|
| Program ROM | 0x000–0x4FF | 1280 bytes | 4001 #0–#4 (chip select = address bits 11:8) | firmware |
| Program ROM (architectural max) | 0x000–0xFFF | 4096 bytes | up to 16 × 4001 | headroom for a Rust build that does not fit 1280 bytes |
| RAM main characters | 2 chips × 4 registers × 16 nibbles | 128 nibbles (64 bytes) | 4002 #0, #1 (CM-RAM0, P0 = 0/1) | working registers, memory |
| RAM status characters | 2 × 4 × 4 nibbles | 32 nibbles | same | flags, sign, decimal position |
| CPU index registers | R0–R15 | 16 nibbles | 4004 | scratch, pointers |
| CPU stack | 3 levels | — | 4004 | subroutine return addresses only, not addressable |

## Budgets

- ROM: **1280 bytes** to match the original; 4096 bytes is the hardware ceiling with more 4001s. The title report states size against both.
- RAM: 160 nibbles (128 main + 32 status) plus 16 index registers. Call depth: 3, including any interrupt-free helper the backend emits.

Sources: MCS-4 data sheet (Nov 1971) and the ai_hw_engineer board spec §2 (chip count and straps).

## Linker regions (`link.ld`)

The CPU cannot flat-address data RAM; a datum is reached by an `SRC` pointer
into `{chip, reg, char}` (`llvm-i4004/spec/abi.md` §1.3), so the linker only
carves the writable space into two non-overlapping regions, modelled as the
backend's 16-bit AS-0 handle `[bank:2][chip:2][reg:2][char:4]`:

| Region | Handle range | Device | Size | Use |
|---|---|---|---|---|
| `rom` | 0x000–0x4FF | 4001 #0–#4 | 1280 B | `.reset` (at 0), `.text`, `.rodata`, `.data` load image |
| `ram` | 0x0000–0x003F | 4002 #0 main | 64 nibbles | `.data` (copied from ROM by crt0) + `.bss` arena |
| `stack` | 0x0040–0x007F | 4002 #1 main | 64 nibbles | software return stack (abi.md §4.4, P7/SSP, grows upward) |

- The software return stack is kept in a **separate 4002** from the `.data`/
  `.bss` arena so a deep call graph cannot corrupt live variables. `__sstack_base`
  / `__sstack_top` bound it; crt0 initialises P7 (SSP) to `__sstack_base`.
- The Busicom firmware's own call graph is ≤ 3 deep, so it uses only the 4004
  **hardware** return stack and never touches the software stack region — that
  region is reserved for a Rust build whose static call depth can exceed 3
  (abi.md §4.4 two-tier scheme).
- RAM **status** characters (AS-2) are opcode-addressed, not linker-placed; the
  backend names them directly.
- `link.ld` ends with an `ASSERT` that fails the link if the image exceeds the
  1280-byte five-ROM budget; widen `rom` (and add 4001 straps on the board) only
  for a larger board, up to the 4096-byte architectural max.

## Extended (xl) board

The stock board cannot hold the compiled Rust firmware. The image is 4,095 B
(1,280 B on stock), and the firmware uses all four 4002s of RAM bank 0 (two on
stock). The **xl** variant (`platform/rtl/busicom_141pf_xl.sv`,
`run_image.sh --board xl`, `link_xl.ld`) keeps the Busicom I/O **identical**:
the same keyboard, printer and lamp ports on the same chips. It only adds
memory, up to the architectural maximum: 16 × 4001 ROM and 16 × 4002 RAM. The
firmware uses the ROM and bank 0 only (issue #18); banks 1–3 are unused.

### Chip complement

| Device | Stock | xl | Selection |
|---|---|---|---|
| 4001 ROM | 5 (#0–#4) | **16** (#0–#15) | chip-select = address bits 11:8 |
| 4002 RAM | 2 (bank 0: #0,#1) | **16** (banks 0–3, 4 chips each) | CM-RAM bank line (DCL) + 4002-1/-2 metal option + P0 strap |

The added 4001s (#5–#15) have **no I/O wired** — their ROM-port pins are unused,
exactly as on the stock board's #3/#4. The firmware's ROM-port select aliasing
(`SRC X2 nibble mod 5`, documented in `busicom_141pf.sv`) is preserved on the xl
top so the original ROM's keyboard/printer/drum port usage is byte-identical;
extra ROMs are invisible to a program that never fetches past 0x4FF.

Bank 0's two original 4002s keep their I/O roles: chip 0 is **RAM0** (printer
red / hammer / paper-advance port), and chip 1 is **RAM1** (memory / overflow /
negative lamp port). Their characters hold the calculator registers, as on the
original. Chips 2 and 3 are the added scratch, buffers and frames (map below).

Per bank the four chips are an authentic quad: 4002-1 P0=0 → chip 0, 4002-1 P0=1
→ chip 1, 4002-2 P0=0 → chip 2, 4002-2 P0=1 → chip 3 (`intel_4002` answers
`{Variant1, po_i}` under its asserted `cm_ram_i`).

### Extended memory map

| Space | Range | Size | Devices | Use |
|---|---|---|---|---|
| Program ROM | 0x000–0xFFF | 4096 bytes | 4001 #0–#15 | the flat image (4,095 B used) |
| RAM main characters | 16 chips × 4 reg × 16 nibbles | 1024 nibbles (512 bytes) | 4002 ×16, banks 0–3 | bank 0 used (256 nibbles, map below); banks 1–3 spare |
| RAM status characters | 16 × 4 × 4 nibbles | 256 nibbles | same | bank 0: signs, scales, flags, FIFO/selector/key state |

### Bank-0 map used by the firmware (issue #18)

A RAM address is the `SRC` byte `[chip:2][reg:2][char:4]`, so bank 0's 256
main characters form one linear 8-bit space (`firmware/src/ram.rs`). Each row
below is one register: 16 main characters plus status characters S0–S3. No
`DCL` is ever issued; CM-RAM bank 0 is the reset default.

| Register | Chip | Main characters | Status characters |
|---|---|---|---|
| 0x00 `WR` working | 0 | 16 BCD digits, least significant first | S0 sign, S1/S2 scale (low/high), S3 flag `ENTERING` |
| 0x10 `DR` dividend/multiplicand | 0 | digits | sign, scale, S3 `POINT` |
| 0x20 `CR` constant | 0 | digits | sign, scale, S3 `OP_FROM_ENTRY` |
| 0x30 `SR` subtotal | 0 | digits | sign, scale, S3 `MODE` |
| 0x40 `TR` main total | 1 | digits | sign, scale, S3 `MOP` |
| 0x50 `MR` memory | 1 | digits | sign, scale, S3 `LAMP_MEMORY` |
| 0x60 `T1` temporary | 1 | digits | sign, scale, S3 `LAMP_OVERFLOW` |
| 0x70 `T2` temporary | 1 | digits | sign, scale |
| 0x80–0xBF scratch | 2 | multiply: product `MULBUF` 0x80 (32 digits). Divide: remainder `DIVREM` 0x9F (17 digits), quotient window `WIN` 0xB0. Square root: remainder `SQ_REM` 0x80 and odd term `SQ_S` 0x94 (20 digits each), root in `WIN` | WIN's sign/scale |
| 0xC0 key FIFO | 3 | slot *i*: chars *i* (low) and 8+*i* (high) of `code − 129` (host only) | S0 head, S1 count (host only); S2 `SW_DP`, S3 `SW_ROUND` (latched by every scan) |
| 0xD0 frames | 3 | compiler spill frames (packer overlay, `frames` region) | — |
| 0xE0 print register `PR` | 3 | chars 0–14: each column's drum sector (0–9, 10 point, 12 minus, 15 blank) | S0/S1 symbol A/B sector, S2 bit 0 red, S3 red latch (host driver) |
| 0xF0 frames | 3 | compiler spill frames (`frames2` region) | S0 `KB_DOWN` (a key was down on the last scan pass) |

The minus lamp is `WR`'s sign (S0 of 0x00); there is no separate flag.

### Linker regions (`link_xl.ld`)

| Region | Range | Size | Use |
|---|---|---|---|
| `rom` | 0x000–0xFFF | 4096 B | `.reset` (at 0), `.text`, `.rodata` |
| `frames` | 0x00D0–0x00DF | 16 chars | per-function spill frames (packer overlay) |
| `frames2` | 0x00F0–0x00FF | 16 chars | more frames |
| `ram`, `stack` | — | 0 | empty: the firmware has no `.data`/`.bss` and no software stack; the link fails if a static appears |

- The firmware lays out the rest of bank 0 itself (table above), not the
  linker.
- `link_xl.ld` ends with `ASSERT(. <= 0x1000, …)`, the 4096-byte ceiling. The
  packer also fails the link if a call chain exceeds the 3-level hardware
  stack.

## Extended (xl+bank) ROM bank switching — a DEVIATION from the original hardware

**Frozen (2026-10-22).** This section describes the 2026-10-21 banked build
(~100 KiB, 0/42). Since issue #18 the firmware is a flat 4 KB image that never
writes the latch. The machinery remains in the packer and the chips adapter
but receives no work.

The Rust firmware compiled by `llvm-i4004` is far larger than the 4096-byte
architectural ROM maximum (the 4004's 12-bit program counter addresses only
16 × 4001 = 4 KiB). To run it at all we add a **ROM bank latch**, which the
original Busicom 141-PF did NOT have. This is an explicit, documented hardware
deviation; the original ROM never switches banks and runs identically with the
latch tied to bank 0.

### The latch (8-bit, two ROM output ports)
- The bank selector is an **8-bit latch** split across **two** 4001 output
  ports: the LOW nibble on **chip #3's port** (`SRC 0x30; WRR`) and the HIGH
  nibble on **chip #4's port** (`SRC 0x40; WRR`). `bank = (chip4 << 4) | chip3`,
  selecting banks 0..255. An earlier design used a single 4-bit nibble on chip
  #3 and capped the image at 16 banks; the Rust firmware needs 45+, so the
  latch was widened. Chips #0–#2 keep their Busicom port roles; chips #3 and #4
  are unwired on the original board, so writing them perturbs nothing on the
  original ROM.
- The 8-bit latch is mirrored into two RAM chars, `__cur_bank.lo` @ 0x0F and
  `__cur_bank.hi` @ 0x0E of chip0/reg0, so a return can restore the caller bank.
- **Reset / crt0** clears both mirror chars and both latch ports to bank 0.
- Every latch write runs ONLY from the common region (`__soft_ret`), writing
  the HIGH nibble first then the LOW nibble; the low-nibble write is the commit
  a simulator triggers the ROM reload on.

### Address map (common + banked)
- **Common region (always mapped from bank 0).** Holds `.reset`/crt0, the HAL
  glue, the hot libi4004rt helpers, the soft-call return dispatch (`__soft_ret`,
  `__srtab`, the push helpers `__push_off/__push_pg/__push_pb/__push1`,
  `__save_p4p5`, the per-page `jin p0` stubs) and any far-thunk fallbacks, so
  the code that performs a bank switch never unmaps itself. Sized by
  `--common-bytes` (0x900 for the current firmware: must leave `bank_bytes`
  room for the largest single function under the 4096-byte per-bank view).
- **Banked region.** Mapped from the bank the latch selects. `--bank-bytes`
  per bank (0x700 for the current firmware; common + bank must be ≤ 4096, the
  4004's 12-bit view). Total ROM = common + N × bank (N=55, ~100 KiB now).

### Cross-bank call ABI (far-call thunks + forward dispatch)
A call to a different bank is NOT a direct jump (the target is not mapped until
the latch is set, and a bank switch from banked code would unmap the running
instruction). The packer rewrites each cross-bank `jun callee` to `jun thunk`
where the thunk lives in the CALLER's bank (or common on a full bank) and:
1. `JMS __save_p4p5` — save P4:P5 (the 2nd i16/ptr arg pair) to RAM;
2. push a FORWARD token `[tramp.off.lo, off.hi, page, callee_bank.lo,
   callee_bank.hi]` via `__push_off` + `__push_pb` + `__push1`;
3. `jun __soft_ret` (COMMON), which pops the token, drives BOTH latch ports to
   the callee bank, and dispatches to the callee's restore trampoline (in the
   callee bank), which reloads P4:P5 and `jun`s the real callee.
The callee returns via its own `jun __soft_ret`, which pops the 5-nibble RETURN
token (pushed by the CALLSOFT prologue, bank from `__cur_bank`) and restores the
caller's 8-bit bank. The thunk never touches the latch, so no running
instruction is swapped out. This replaces the old `__bankcall_*` JMS trampoline
(which cost a hardware-stack slot); the soft-call path has no such limit.

### Packer bank assignment
`i4004_pack.py --banks N --common-bytes C --bank-bytes B` classifies common vs
banked functions, bin-packs the banked ones call-graph-aware (keep a callee near
its caller to cut cross-bank calls), assigns per-function RAM frames (bank-0
overlay), synthesizes the dispatch + far thunks, and rewrites cross-bank calls.
Image file = `common(C) || bank0(B) || bank1(B) || …`.

### Simulator support (both must honour the 8-bit latch)
- **chips adapter** (`platform/scripts/chips_trace.rs`): the unmodified
  `external/chips` model has no bank concept, so the adapter reads
  `bank = (roms[4].ports << 4) | roms[3].ports`, triggering on the chip-3
  (commit) write, and rebuilds the banked ROM chips (index ≥ `common/256`) from
  the selected bank's window. The latch-port chips (#3, #4) lie in the common
  region so they are never swapped out.
- **RTL xl top** (`platform/rtl/busicom_141pf_xl.sv`): NOT YET banked. Today it
  instantiates 16 fixed 4001s covering a flat 4 KiB, so it can run only images
  that fit 16 banks' worth at the 4 KiB view — NOT the 100 KiB firmware. To run
  the banked firmware the RTL needs:
  1. an 8-bit **bank register** that captures `WRR` to chip #3's port (low
     nibble) and chip #4's port (high nibble), cleared to 0 on reset;
  2. a **banked ROM backing store** larger than 4 KiB (a memory initialised
     from the full `common || bankN...` image, 45+ banks ≈ 100 KiB) addressed
     as `common[addr]` for `addr < common_bytes` else
     `banked[bank*bank_bytes + (addr - common_bytes)]`;
  3. the ROM data-mux for the banked address range to read that store at the
     current bank, while the common range and the chip #0–#2 I/O ports stay
     unbanked. Only the fetch path changes; the port/translator logic is
     untouched. This mirrors the chips adapter exactly and is the remaining
     work to validate the firmware on the authoritative RTL once it passes on
     chips.

The original ROM (1280 B) never touches chip #3's port, so both runners reproduce
`rom_port_trace.txt` byte-for-byte with the latch present (it stays at bank 0).

Sources: MCS-4 data sheet (Nov 1971); 4002 chip-select (metal option + P0 + CM-RAM
line) and DCL bank decode from `../ai_hw_engineer/design/intel_4002/src/intel_4002.sv`
and `.../intel_4004/src/intel_4004.sv`; derived xl board top
`platform/rtl/busicom_141pf_xl.sv` cites `../ai_hw_engineer/system/busicom_141pf/src/busicom_141pf.sv`.
