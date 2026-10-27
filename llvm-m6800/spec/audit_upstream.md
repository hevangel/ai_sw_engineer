# Upstream compiler support audit — Motorola MC6800

Audited 2026-10-26 for hevangel/ai_engineer#61. Sources linked inline; version
claims are as of that date.

## Summary

**No LLVM, GCC or Rust implementation targets the MC6800.** The available
tooling is assemblers (vasm's 6800 module, Macro Assembler AS, Motorola's
classic cross assemblers) plus whole-machine emulators. A working Rust/LLVM
target therefore requires the project-owned-backend route proven by
llvm-i4004/llvm-i8008. Per the issue: a 6809 or 68k target is **not
interchangeable** — different instruction encodings, addressing modes,
register sets and endian-adjacent conventions.

## LLVM

- No 6800 backend has ever been proposed or merged upstream. LLVM's
  `M68k` target is the **680x0 32-bit family (68000+)** — a different
  architecture in every layer (32-bit registers, entirely distinct
  encodings and addressing); it is the subject of ai_engineer#64, not this
  one. Searched LLVM targets/ and the forks index, 2026-10-26: nothing.

## GCC

- GCC has never had an MC6800 backend (its Motorola coverage starts at
  m68k/68HC11 variants of the 68020-class family). No usable fork found.

## Assemblers (the usable existing tooling)

- [vasm 6800 CPU module](http://sun.hasenbraten.de/vasm/release/vasm_33.html)
  — maintained by Frank Wille (module by Esben Norby/Frank Wille,
  2013–2021); the practical cross-assembler choice. Freely redistributable
  (vasm's custom free license).
- [Macro Assembler AS](https://assembler.cp-o.com/) (`as6800`) — long-lived
  multi-target assembler, ANSI C source.
- [JimInCA/motorola-6800-assembler](https://github.com/JimInCA/motorola-6800-assembler)
  — Motorola's classic cross assemblers (as0/as1) rebuilt.
- [glitchwrks/a68](https://github.com/glitchwrks/a68) — small C99 assembler.
- Survey context: ["Choosing the wrong M6800 assembler"](https://tobiasvl.github.io/blog/choosing-the-wrong-m6800-assembler).

These give the future backend two independent encoder references and, via
the emulators below, execution targets for hand-assembled probes (the
llvm-i4004 route's first execution milestone).

## C compilers

- No maintained MC6800 C compiler exists. (OS-9's C compiler was 6809;
  historical 6800 Small-C derivatives are museum pieces.) Assembly is the
  common route on this architecture.

## Emulators / machines (oracle side)

- [MAME](https://github.com/mamedev/mame) ships a SWTPC 6800 driver
  (`swtpc/swtpc.cpp`) — a whole-machine oracle for MIKBUG/SWTBUG software.
- Standalone SWTPC emulators (e.g. [torok.info SWTPC](https://www.torok.info/computing/SWTPC/index.htm));
  software archive at [deramp.com/swtpc.html](https://deramp.com/swtpc.html).
- **Primary executor for this project**: the ai_hw_engineer MC6800 RTL
  (`design/motorola_6800`) — full documented ISA against a PRM-derived
  Python ISS (1,951/1,951 vector checks) plus 50 vendored third-party
  self-checking programs; **fresh full-ISA PASS re-run 2026-10-26** (see
  [../report/audit_2026-10-26.md](../report/audit_2026-10-26.md)).

## Rust

- No 6800 target exists in rustc's target list, rust-lang/compiler-contrib,
  or the community custom-target ecosystem (searched 2026-10-26). The
  `m6800-none` target JSON will be project-owned, in the llvm-i4004 style,
  after the backend exists.

## Consequence for the profile (issue #61)

The configuration is a from-scratch backend on the pinned LLVM tree. The
compiler contract is fixed in [compiler_profile.md](compiler_profile.md);
the staged plan in [../plans/implementation_plan.md](../plans/implementation_plan.md)
follows the llvm-i4004/i8008 route. No implementation is claimed.
