# Upstream compiler support audit — Motorola MC6809

Audited 2026-10-26 for hevangel/ai_engineer#62. Sources linked inline;
version claims are as of that date.

## Summary

**No LLVM or Rust implementation can target the MC6809 today.** LLVM's only
6809 attempt (SourceForge `llvm6809be`, LLVM 2.x era) is abandoned and never
landed upstream; GCC's gcc6809 (4.3.6-era) and CMOC are C compilers — usable
as references, not as a Rust path. The practical reuse targets for this
project are gcc6809/LWTOOLS as ABI and encoder references, and a
project-owned backend following the llvm-i4004/i8008 route. The 6309
(Hitachi superset) and 68HC11 are out of scope; the plain MC6809 is the
exact variant.

## LLVM

- [M6809/H6309 Back-End for LLVM](https://sourceforge.net/projects/llvm6809be)
  (SourceForge) — an out-of-tree backend from the LLVM 2.x era, last
  activity ~2014, never proposed upstream. Abandoned; not reusable against
  the pinned LLVM 23 tree beyond as a reading reference.
- Upstream LLVM `M68k` targets the 680x0 32-bit family — a different
  architecture (issue ai_engineer#64), not interchangeable (same note as
  issue #61 makes for the 6800).

## GCC / C compilers (the strong existing references)

- **gcc6809** — a real optimizing GCC backend for the 6809, based on GCC
  4.3.6 ([jmatzen/gcc6809](https://github.com/jmatzen/gcc6809),
  [dfffffff/gcc6809](https://gitlab.com/dfffffff/gcc6809); originally a
  2011 Google Code project). Unmaintained but functional; the community
  pairs it with LWTOOLS as assembler/linker. Its calling convention,
  register allocation notes (X/Y/U versus the 6800's single index
  register) and DSP/TFR usage are the primary ABI reference for this
  project.
- **CMOC** (Pierre Sarrazin) — an actively maintained C subset compiler
  generating 6809 code (recent 0.1.100-line releases; [project
  page](http://gvlsywt.cluster051.hosting.ovh.net/dev/cmoc.html)). Useful
  as a codegen comparison oracle.
- Comparison material: ["CMOC vs VBCC vs GCC6809"](https://vectrex.substack.com/p/cmoc-vs-vbcc-vs-gcc6809)
  and Aslak's ["Exploring C compilers for the 6809"](https://www.aslak.net/index.php/2018/03/07/exploring-c-compilers-for-the-6809).

## Assembler / linker

- [LWTOOLS](https://www.lwtools.ca) — `lwasm`/`lwlink` for 6809/6309,
  **actively maintained (4.24, 2025-04-18)**; the practical encoder
  cross-check and the linker-script model for OS-9/DECB formats.
- vasm's 6809 module (same family as its 6800 module).

## Emulators / machines (oracle side)

- [XRoar](https://www.6809.org.uk/xroar/) (Dragon 32/64, CoCo) — Tormod
  Volden's cycle-reference emulator; MAME covers CoCo 1/2/3 and Vectrex.
- **Primary executor for this project**: the ai_hw_engineer MC6809 RTL
  (`design/motorola_6809`) — full documented ISA verified against the
  hardware-validated reyco2000 self-checking suites plus formal
  BMC/prove/cover; **fresh functional + interrupt PASS re-run 2026-10-26**
  (see [../report/audit_2026-10-26.md](../report/audit_2026-10-26.md)).

## Rust

- No 6809 target exists in rustc's target list or the community
  custom-target ecosystem (searched 2026-10-26). `m6809-none` will be
  project-owned after the backend exists.

## Consequence for the profile (issue #62)

Reuse here means reusing knowledge and references (gcc6809's ABI, LWTOOLS
encodings, CMOC as a comparison oracle), not a compiler. The contract is
fixed in [compiler_profile.md](compiler_profile.md); the staged plan in
[../plans/implementation_plan.md](../plans/implementation_plan.md). No
implementation is claimed.
