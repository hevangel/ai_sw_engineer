# IBM PC AT 5170 — compiler contract (stage-1 specification)

For hevangel/ai_sw_engineer#66. Machine specification and SystemVerilog
reconstruction are owned upstream (hevangel/ai_engineer#142); this document
fixes only what a compiler/linker/runtime for the machine must honor.
Historical references: *IBM PC AT Technical Reference* (1984), *Intel iAPX
286 Programmer's Reference Manual* (1985). Facts here are the standard,
well-documented machine contract; the SV issue owns anything board-specific
beyond it.

## CPU and modes

- Intel 80286, 16-bit external data bus, 24-bit physical address space
  (16 MiB), 6 MHz on the original 5170 (8 MHz model 319 later).
- **Supported modes, in delivery order:**
  1. **Real mode** (reset state, 8086-compatible addressing: segment<<4 +
     offset, 1 MiB window) — first milestone; every DOS-era program and the
     BIOS run here.
  2. **Protected mode** (16-bit selectors, GDT/LDT/IDT, no paging) — second
     milestone, only as sourced software requires it (per the roadmap issue).
- Forbidden instructions for exact-ISA compliance: everything introduced with
  the 80386 and later — 32-bit operand forms (66/67 prefixes are meaningless
  on the 286), `movzx`/`movsx`, `setcc`, `shld`/`shrd`, `bt`/`bts`/…,
  `bswap`/`cmpxchg`/`xadd`, FS/GS segments, 386 control-register moves and
  paging (`PG`), 32-bit protected-mode gates. 8086 + 80186 + 80286
  instructions (including `push imm`, `pusha`/`popa`, immediate shift counts,
  `imul` imm, protected-mode `lgdt`/`lidt`/`lmsw`/`arpl`/`lar`/`lsl`,
  `clts`, `verr`/`verw`) are in scope by mode milestone. 80287 coprocessor
  (x87 in 16-bit form) is an optional peripheral profile — floating point
  stays out of the default ABI (soft-float only), matching the 4004/8008
  profiles' integer-only policy until a sourced consumer needs the 80287.

## Data layout and ABI (planned `i286-none` target)

- Little-endian; `i8`/`i16` native, `i32`/`i64` expanded (register pairs),
  pointers 16:16 far / 16-bit near — the target JSON's address spaces will
  separate near data, far data and code segments, mirroring the ia16/Watcom
  memory models rather than assuming flat 32-bit x86 conventions.
- Calling convention: to be fixed in `spec/abi.md` before any TableGen, with
  gcc-ia16/Watcom conventions as the reference oracle. Candidate default:
  near callee-saves, args in `AX/CX/DX/BX` order then stack, `SS=DS` small
  model; far forms as explicit address-space variants.
- Stack: `SS:SP`, full-descending; the 286 hardware stack is adequate (no
  software return-stack machinery like the 4004 needs).
- Interrupt/vector integration: real-mode IVT at 0:0 (256 × far pointers);
  the runtime provides vectors only as far as the machine contract requires
  (the BIOS owns most of them; a bare-metal profile owns its own).

## Entry, memory and executable formats

- Reset/entry: power-on jumps through `F000:FFF0`; ROM extension contracts
  (option ROMs) check the `0x55AA` signature + length byte + checksum on
  512-byte boundaries.
- Conventional memory 0–640 KiB; BIOS data at 0x400–0x4FF; BIOS ROM at
  `F0000–FFFFF`; the AT's first 1 MiB minus ROM/VRAM holes plus extended
  memory above 1 MiB (24-bit addresses reachable only in protected mode or
  via BIOS int 15h 87h/88h doors — real-mode code cannot use it directly).
- **Package formats the profile must link:**
  - DOS **MZ executable** (relocations applied by the loader) and COM images;
  - flat/binary stage images for bare-metal boot sectors (origin 0x7C00);
  - **ROM/option-ROM** images: 512-byte-aligned, `0x55AA` + checksum.
- Size/relocation checks belong in the packer/linker stage (`link.plans`),
  exactly like `i4004_pack.py`'s budget gate.

## Machine I/O the runtime must know about (contract level)

- Two 8259A PICs (IRQ 0–15, cascaded at IRQ 2), 8254 timer, 8042 keyboard
  controller, 8237A DMA, MC146818 CMOS/RTC — the runtime/HAL contract treats
  these as the BIOS-mediated or port-I/O surface; application/OS software
  belongs to the software issue, not this profile.

## Out of scope for this profile

- OS/application rewrites (software issue), board-accurate SV behavior
  (ai_engineer#142), and any 80386+ feature — the PC AT 5170 shipped with a
  286; the 386-class machines are separate issues.
