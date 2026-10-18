# llvm-i4004 reference material

Sources used to write `spec/isa.md` and `spec/abi.md`. Copyrighted manuals are
**not committed** (per AGENTS.md "never commit a copyrighted ROM/manual"); they
are recorded here by URL and SHA-256 and read from the sibling
`ai_hw_engineer` checkout.

| Reference | Used for | Source | Bytes | SHA-256 |
|---|---|---|---|---|
| MCS-4 Micro Computer Set Users Manual *(not committed)* | instruction repertoire, encodings, cycles, JCN conditions, SRC/DCL command-line, 3-level stack, WPM | public scan [codeabbey mirror](http://codeabbey.github.io/heavy-data-1/msc4-manual.pdf); local copy at `ai_hw_engineer/references/MCS4_Data_Sheet_Nov71.pdf` | 11,184,923 | `4672da4d81b75c98ef7c8892c9f33d522aa55922d48d6cd4952ecffe9a739622` |

Non-copyrighted cross-checks (cited inline in `spec/isa.md` §10):

- ai_hw_engineer Intel 4004 design spec — `design/intel_4004/spec/spec.md` (the
  authoritative behavioral reconstruction; RTL in `src/intel_4004.sv`).
- [e4004 instruction set](http://e4004.szyc.org/iset.html),
  [pastraiser opcode table](https://pastraiser.com/cpu/i4004/i4004_opcodes.html),
  [Pyntel4004 opcodes](https://pyntel4004.readthedocs.io/en/latest/intro/opcodes.html).
- [MAME MCS-40 core](https://github.com/mamedev/mame/blob/master/src/devices/cpu/mcs40/mcs40.cpp) —
  semantics cross-validation (subtract carry, JCN bit order, WPM, TCS/DAA).
- `external/chips` `src/mcs4/cpu4004.rs` — independent emulator, **read-only,
  not copied**.

No source text is reproduced verbatim beyond device names, signal names, and
instruction mnemonics.
