# IBM PC XT 5160 CPU/ABI profile

Acceptance box 2 of ai_sw_engineer#59. Machine facts follow the primary
hardware authority (IBM April 1983 5160 Technical Reference, via
`ai_hw_engineer/system/ibm_pc_xt_5160/spec/spec.md`); every **(P)** paragraph
is provisional and owned for final say by the `llvm-i8086` backend
(ai_engineer#60) — its `spec/abi.md` will supersede the (P) parts, and this
file tracks it. Machine-constraint parts (no marker) are this issue's.

## 1. Exact machine and CPU

- CPU: Intel 8088, nominal 4.772727 MHz, 8-bit external bus, 16-bit
  registers, 20-bit addressing (1 MiB physical).
- Mode: **real mode only**. No protected mode (no 80286), no virtual-8086.
- RAM 00000h–3FFFFh (256 KiB); CGA B8000h–BFFFFh; fixed-disk option ROM at
  C8000h; Cassette BASIC 1.10 + BIOS F6000h–FFFFFh. Full map:
  `platform/memory_map.md`.
- An Intel 8087 is installed (equipment bit set). Its presence does not
  change codegen policy (§4).

## 2. ISA allowlist and forbiddens

Codegen may emit **8086/8088 instructions only**, 8- and 16-bit operand
forms, as established by the manufacturer opcode map (pin the exact Intel
manual edition when the backend's MC tests are written).

Forbidden on the 8088 (assembler/linker and the Capstone check in
`plans/testplan.md` must reject them):

- 80186+: `PUSHA/POPA`, `IMUL` with immediate, shifts/rotates by immediate
  other than 1, `ENTER/LEAVE`, `BOUND`, `INS/OUTS`.
- 80286+: protected-mode instructions and behavior.
- 80386+: any 32-bit register use and the 66h/67h prefixes. This is exactly
  what upstream LLVM `-m16`/`code16` emits (`spec/toolchain_audit.md`), so
  upstream x86 codegen is unusable here.
- NEC V20/V30 and 8087 ESC opcodes, unless a title justifies them and the
  backend gates them behind an explicit CPU/feature flag.

## 3. Endianness, addresses, memory model

- Little-endian throughout.
- Near pointer = 16-bit offset. Segments are a link/load-time concern, not
  IR address spaces: initially one address space (0), one 64 KiB view.
- Memory model: **near-only first** (tiny/small): a single segment holds
  code+data+stack (boot sector, `.COM`), or CS/DS/SS fixed per image kind
  with near calls and near data pointers (§6). Far code/data pointers
  (medium/compact/large/huge à la gcc-ia16) are explicitly out of scope
  until a title needs >64 KiB of code or data (ASSUMPTION XT-A1).
- Physical address = `segment*16 + offset`, 20-bit wrap at 1 MiB (no A20
  gate on the XT).

## 4. Integer and floating-point policy

- Native widths: `i8`, `i16`. `i32` lowers to 16-bit sequences plus backend
  runtime helpers (multiply/divide/shifts), like `llvm-i8008/rt/`.
- `i64`/`i128` are rejected until a title needs them (AGENTS.md width rule).
- No floating-point codegen initially, although the 8087 is installed: no
  title so far needs it, and soft-float helpers are backend work. A title
  whose original used the 8087 (or multi-precision math) may opt into x87
  codegen behind an explicit feature (ASSUMPTION XT-A2).

## 5. Data layout (P)

Proposed LLVM data layout for the near-only profile:

```
e-m:e-p:16:16-i8:8-i16:16-i32:16-i64:16-n8:16-S16
```

- `e`: little-endian. `m:e`: ELF mangling (object files are ELF32).
- `p:16:16`: near pointers are 16 bits, 2-byte aligned.
- `i8:8`, `i16:16`: native widths, natural alignment. `i32:16`, `i64:16`:
  wider integers keep 2-byte alignment (no 4-byte alignment requirement on
  the 8088 bus); `i64` exists in the layout only so IR is well-formed,
  codegen still rejects it (§4).
- `n8:16`: native integer widths. `S16`: stack slots are 2 bytes (every
  `PUSH` is a word); the stack stays 2-aligned.
- No red zone: BIOS/NMI and interrupt handlers share the stack.

`rust/i8086-xt-none.json` carries this string verbatim; any backend change
updates both in the same change.

## 6. Calling convention and stack (P)

Proposed cdecl-near convention (gcc-ia16 small-model compatible in spirit):

- Arguments on the stack, pushed right to left, 2 bytes each (`i8` widens
  to a word slot); the caller cleans up.
- Return: `i8`/`i16` in `AX` (`i8` in `AL`, upper byte undefined);
  `i32` in `DX:AX`; larger via hidden `sret` pointer (first stack slot).
- Clobbers: `AX`, `CX`, `DX`, `BX`, `SI`, `DI`, `BP`-relative addressing
  aside, `ES`, and the flags are caller-saved (all clobbered). `BX`, `SI`,
  `DI`, `BP` callee-saved is the alternative the backend may choose for
  smaller Rust frames; the decision and the saved-register list land in the
  backend's `abi.md` and are mirrored here.
- `DF` is 0 across every call. `SS` equals `DS` in tiny-model images; the
  crt0 establishes the documented segment values per image kind (see
  `platform/runtime/README.md`).
- Hardware stack (`SS:SP`), grows down, 2-byte slots. No static frames:
  unlike the 4004/8008, the 8088 has a real stack, so recursion and
  reentrancy are allowed by the ABI; each title still budgets stack in
  `platform/memory_map.md`.

## 7. Object format and relocations (P)

- In: ELF32 relocatable objects, `ELFOSABI_STANDALONE`. The `EM_` machine
  number is TBD by the backend (no registered 8086 value; ia16-elf proves
  ELF works for 16-bit objects).
- Out: flat binary via a project-owned linker/packer (repo precedent:
  `llvm-i8008/asm/i8008_link.py`), then packaged as boot sector, `.COM`,
  or MZ `.EXE` (§8).
- Relocations needed at minimum: 16-bit absolute (`R_I8086_16` sketch),
  8-bit low/high halves for immediates, and PC-relative 8/16-bit branch
  fixups. Exact numbers are backend-owned.

## 8. Image kinds and entry points

| Kind | Load address | Entry state | Size cap |
|---|---|---|---|
| Boot sector | `0000:7C00` | `DL` = boot drive; segments/flags per BIOS; ends with `55 AA` | 510 code bytes |
| DOS `.COM` (tiny) | `CS:0100h`, `CS=DS=ES=SS` | `SP` = top of segment; `FCB`/`DTA` set by DOS | 64 KiB − 256 B − stack |
| DOS MZ `.EXE` (small, later) | relocated per header | stack/registers per MZ header; `DS`/`ES` = PSP | headers + segments |

Boot sector first (no DOS dependency, directly bootable on both oracles);
`.COM` second; MZ only when DOS-hosted titles need it.

## 9. Interrupt and vector integration

- IVT at physical `00000h`, 256 far vectors. Bare-metal titles may claim
  vectors by writing the IVT (and masking the PIC); DOS-hosted titles go
  through DOS/BIOS services. Reserved: CPU vectors 0–7, BIOS `10h`–`1Ah`,
  DOS `20h`–`2Fh`, XT fixed-disk `13h` chain.
- Title HALs expose interrupt use explicitly; the crt0 never installs
  handlers silently.

## 10. Assumption ledger

| ID | Statement | Validation |
|---|---|---|
| XT-A1 | Near-only (≤64 KiB code, ≤64 KiB data+stack) suffices for the first titles | First title size report; revisit when an image exceeds it |
| XT-A2 | No FP codegen until a title's original needs the 8087 | Title spec cites original's 8087 use before enabling |
| XT-A3 | `DF=0` across calls; no red zone | Backend lit tests + a stack-clash smoke test on the oracles |
| XT-A4 | ELF32 relocatable in / flat binary out fits the backend | Backend `codegen.md` decision; this file mirrors it |
