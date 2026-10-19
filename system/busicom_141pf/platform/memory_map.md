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

The stock board cannot hold the compiled Rust firmware: the runtime + crt0 +
HAL alone take ~697 of the 1280 ROM bytes, and the firmware's static state
(Calculator 68 B + KeyBuffer 24 B = 92 B, before stack/spills) already exceeds
the stock 64-byte main RAM of the two bank-0 4002s. The **xl** board variant
(`platform/rtl/busicom_141pf_xl.sv`, `run_image.sh --board xl`, `link_xl.ld`)
keeps the Busicom I/O **identical** — same keyboard, printer and lamp ports on
the same chips — and only ADDS memory, to the architectural maximum where it is
cheap: 16 × 4001 ROM and 16 × 4002 RAM. Shrinking back onto the stock board is
a later optimisation; this variant exists to get the firmware running first.

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

Bank 0's two original 4002s keep their roles: chip 0 = **RAM0** (printer red /
hammers / paper-advance port) and chip 1 = **RAM1** (Memory / Overflow /
Negative lamp port). Their main-character storage stays reserved for the
firmware working set, matching the stock layout, so linker-placed data starts at
the first ADDED chip.

Per bank the four chips are an authentic quad: 4002-1 P0=0 → chip 0, 4002-1 P0=1
→ chip 1, 4002-2 P0=0 → chip 2, 4002-2 P0=1 → chip 3 (`intel_4002` answers
`{Variant1, po_i}` under its asserted `cm_ram_i`).

### Extended memory map

| Space | Range | Size | Devices | Use |
|---|---|---|---|---|
| Program ROM | 0x000–0xFFF | 4096 bytes | 4001 #0–#15 | firmware (runtime+crt0+HAL+logic) |
| RAM main characters | 16 chips × 4 reg × 16 nibbles | 1024 nibbles (512 bytes) | 4002 ×16, banks 0–3 | working set, `.data`/`.bss`, software stack, heap headroom |
| RAM status characters | 16 × 4 × 4 nibbles | 256 nibbles | same | flags / opcode-addressed |

ROM: **4096 bytes** (16-ROM architectural ceiling). RAM: **1280 nibbles total**
(1024 main + 256 status) = 640 bytes, up from the stock 160 nibbles.

### Linker regions (`link_xl.ld`)

Same AS-0 handle model `[bank:2][chip:2][reg:2][char:4]` as the stock script.

| Region | Handle range | Device | Size | Use |
|---|---|---|---|---|
| `rom` | 0x000–0xFFF | 4001 #0–#15 | 4096 B | `.reset` (at 0), `.text`, `.rodata`, `.data` load image |
| `ram` | 0x0020–0x005F | bank 0 chips 2–3 main | 128 nibbles | `.data` (copied from ROM by crt0) + `.bss` arena |
| `stack` | 0x0100–0x013F | bank 1 (4 chips) main | 256 nibbles | software return stack (abi.md §4.4, P7/SSP, grows upward) |
| (spare) | 0x0200–0x03FF | banks 2–3 main | 512 nibbles | heap/arena headroom; add a named region when a build needs it |

- Bank 0 chips 0–1 handles (0x0000–0x001F) are **not** linker-placed: they hold
  the firmware's own working set and back the printer/lamp I/O port latches,
  identical to the stock board.
- The `.data`/`.bss` arena and the software stack are in **different CM-RAM
  banks**, so a deep call graph cannot corrupt live variables.
- `link_xl.ld` ends with `ASSERT(. <= 0x1000, …)`, the 4096-byte ceiling.

## Extended (xl+bank) ROM bank switching — a DEVIATION from the original hardware

The Rust firmware compiled by `llvm-i4004` is far larger than the 4096-byte
architectural ROM maximum (the 4004's 12-bit program counter addresses only
16 × 4001 = 4 KiB). To run it at all we add a **ROM bank latch**, which the
original Busicom 141-PF did NOT have. This is an explicit, documented hardware
deviation; the original ROM never switches banks and runs identically with the
latch tied to bank 0.

### The latch
- A 1-nibble latch on **4001 chip #3's output port** (`SRC` chip 3 then `WRR`).
  Chips #0–#2 keep their Busicom port roles; chip #3's port is unwired on both
  the stock and the plain-xl board, so writing it perturbs nothing on the
  original ROM. The latch value (0..15) selects the active ROM **bank**.
- **Reset** clears the latch to bank 0.

### Address map (common + banked)
- **Common region: ROM chips #0–#3 (addresses 0x000–0x3FF, 1 KiB).** Always
  mapped from bank 0 regardless of the latch. Holds `.reset`/crt0, the HAL
  glue, the hot libi4004rt helpers, and the **cross-bank call trampolines**, so
  the code that performs a bank switch never unmaps itself.
- **Banked region: ROM chips #4–#15 (addresses 0x400–0xFFF, 3 KiB per bank).**
  Mapped from the bank the latch selects. Total ROM = 1 KiB common + N × 3 KiB.

### Cross-bank call ABI (trampoline in the common region)
A call whose target is in a different bank does NOT `JMS` the target directly
(the target is not mapped until the latch is set, and setting it would unmap the
caller). Instead the packer rewrites it to `JMS __bankcall_<target>` where the
trampoline lives in the common region and does:
1. push the caller's current bank nibble onto the P7 software stack;
2. set the latch to the target's bank (`SRC p(chip3); LDM bank; WRR`);
3. `JMS target` (now mapped);
4. on return, pop the caller bank and restore the latch (`WRR`);
5. `BBL` back to the caller.
The hardware return stack is still ≤3 deep logically; the trampoline adds one
level, so a cross-bank call costs one extra hardware-stack slot — the packer
accounts for this when it refuses to deepen a graph past 3 (else it routes the
subtree through the §4.4 software return stack).

### Packer bank assignment
`i4004_pack.py --banks N` assigns each function to a bank (call-graph aware:
keep a callee in the same bank as its hot caller where it fits; the common
region is bank-agnostic), lays out 1 KiB common + N × 3 KiB banked, emits the
trampolines, and rewrites cross-bank `JMS`/`JUN` to the trampoline. The image
file is `common(1KiB) || bank0_hi(3KiB) || bank1_hi(3KiB) || …`.

### Simulator support (both must honour the latch)
- **chips adapter** (`platform/scripts/chips_trace.rs`): the unmodified
  `external/chips` model has no bank concept, so the adapter watches
  `b.roms[3].ports`; on a change it rebuilds `b.roms[4..16]` (public `Vec`,
  `ROM::new` is public) from the selected bank's 3 KiB. Chips #0–#3 stay bank 0.
- **RTL xl top** (`platform/rtl/busicom_141pf_xl.sv`): a latch register captures
  `WRR` to chip #3's port and gates the ROM chip-select / data-mux for chips
  #4–#15 to read the selected bank's mask. Chips #0–#3 are unbanked.

The original ROM (1280 B) never touches chip #3's port, so both runners reproduce
`rom_port_trace.txt` byte-for-byte with the latch present (it stays at bank 0).

Sources: MCS-4 data sheet (Nov 1971); 4002 chip-select (metal option + P0 + CM-RAM
line) and DCL bank decode from `../ai_hw_engineer/design/intel_4002/src/intel_4002.sv`
and `.../intel_4004/src/intel_4004.sv`; derived xl board top
`platform/rtl/busicom_141pf_xl.sv` cites `../ai_hw_engineer/system/busicom_141pf/src/busicom_141pf.sv`.
