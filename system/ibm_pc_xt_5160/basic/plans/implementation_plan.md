# IBM PC BASIC rewrite: implementation plan

Issue [#301](https://github.com/hevangel/ai_sw_engineer/issues/301). Phase 1 (this document, the spec, the new
cases) done 2026-11-09. The user accepted its recommendations on 2026-11-10 (§5, decisions recorded there); the
medium-model route moved into the toolchain, and M0 and M1 are done (§3, status column).

## 1. Toolchain findings (measured)

### What the route can build today

- Rust → LLVM IR through the **msp430 route** (`llvm-i8086/scripts/build_rust.sh`; the i8086 rustc of
  `rust_build.sh` is not built in this environment, `/build/rust-obj` is absent), `llc -mtriple=i8086`,
  `i8086-ld`. Tiny model by default: one 64 KiB segment for code, data and stack (`link_com.ld`).
- The backend already has far data pointers (`ptr addrspace(1)`, abi.md D7), far functions (`cc 1000`, D8)
  and several segments with `sym@seg` relocations and an MZ `.EXE` writer (D9). **Rust cannot name either**:
  there is no Rust syntax for address space 1 or for calling convention 1000, and the existing titles reach
  other segments through assembly helpers (`dos_far_read16`, DEBUG's `debug_peek`) — one call per word.
- The backend never emits jump tables (`setMinimumJumpTableEntries(UINT_MAX)`), so `match` dispatch compiles
  to compare trees; nothing in generated code refers to code addresses through data.

### How big the interpreter will be

| Measure | Result |
|---|---|
| Size probe [size_probe/](size_probe/) — tokenizer + lister with the whole keyword table, MBF single and double add/sub/mul/div/compare/convert, FIN, FOUT, a PRINT USING field formatter, a small evaluator; 1,139 lines of Rust | **17,102 bytes of code + 1,600 bytes rodata** (`opt-level = "z"`, fat LTO, `panic = "abort"`); biggest functions: crunch 1,971, FIN 1,658, digit generation 1,580, PRINT USING 1,281, FOUT 1,037, double add 916 |
| Code per source line | about 15 bytes |
| Existing titles, rewrite vs original | DEBUG 20,902 vs 11,904 bytes (1.8×), EDLIN 10,066 vs 4,608 (2.2×) |
| The original | ROM 32,768 + BASIC.COM 16,256 = 48 KiB of hand-written 8086 code and tables |

Estimate for the whole interpreter, by component (the probe's density for the parts it covers):

| Component | KB |
|---|---|
| MBF arithmetic, conversions, FIN/FOUT/PRINT USING (probe: ~11 KB, to be completed) | 13 |
| Transcendentals, `^`, RND | 4 |
| Tokenizer, lister, keyword tables | 5 |
| Evaluator, variables, arrays, DEF FN, type coercion | 9 |
| Statements (~90) and control flow | 16 |
| Strings and garbage collection | 5 |
| Program editing: insert/delete lines, AUTO, RENUM, DELETE, EDIT, CHAIN/MERGE | 6 |
| Screen editor, key line, KEY, LOCATE, WIDTH, SCREEN function | 9 |
| Files, devices, SAVE/LOAD in three forms, FILES/KILL/NAME, error mapping | 12 |
| Errors and messages | 3 |
| Graphics (PSET, PRESET, LINE, POINT, SCREEN 1/2), SOUND/BEEP, COM/LPT, CALL/USR, BLOAD/BSAVE, SHELL | 10 |
| **Total** | **≈ 90 KB (range 80-110 KB)** |

That is 1.9× the original, in line with DEBUG and EDLIN. **It does not fit in one 64 KiB segment** even with
all data moved out; aiming for a single segment would mean either dropping features or hand-compressing the
Rust beyond what the earlier titles achieved.

### The medium-model proof (done in this phase)

Phase 1's proof (now the toolchain's route, [medium_model/](medium_model/README.md): `llvm-i8086/scripts/build_rust_medium.sh`,
`llvm-i8086/rt/medium/`, test `llvm-i8086/tests/medium/`) built one source twice:

- `tiny.com` (2,110 bytes): no padding, near calls, `link_com.ld`;
- `medium.exe` (120,009 bytes): the same program plus 150 generated functions (~95 KB of code), **every
  function far (`cc 1000`)**, packed by size into three code segments (main 21,698 bytes with 17,598 of code;
  far1 52,280; far2 44,985), a separate 64 KiB data segment from DOS function 48h written and read through
  inline far loads/stores, 64-bit and 32-bit libcalls in every segment, cross-segment mutual recursion,
  six-argument calls (stack arguments above a far return address) and an `sret` struct return.

Both ran under PC DOS 2.00 on MartyPC's IBM 5160 and printed exactly the text of the host build of the same
logic (`FE44A5E2E4723C00 ... 014A 40D12DE0 $`). Code-size overhead of all-far calls: 114,863 bytes in
segments vs 112,988 near (1.7%, including two per-segment runtime copies).

What it took (now in `llvm-i8086/scripts/`, spec/abi.md D10):

1. **`ir_medium.py`**, an IR pass between rustc's IR and `llc`:
   - `--far-intrinsics`: calls to `__far_r8/__far_r16/__far_w8/__far_w16(seg, off[, v])` (declared
     `extern "C"` in Rust) become `inttoptr` + `load`/`store` through `ptr addrspace(1)`, which the backend's
     `I8086FarPointers` expands inline (`push es; mov es, seg; mov al, es:[bx]; pop es`) — no call per byte;
   - `--medium SIZES`: every defined function and every declared non-intrinsic one gets `cc 1000`, so do the
     calls; functions are packed first-fit-decreasing into segments (sizes from a near-model compile, read
     with `llvm-readobj`), with `section ".far.text.<n>.<name>"`; indirect calls and taken addresses are
     refused (16-bit function pointers cannot cross segments).
2. **`seg_rt.py`**: `llc` calls the runtime library (`__udivsi3`, `memcpy`, `__muldi3`, ...) with near calls,
   so each segment n ≥ 1 that calls one gets its own copy of `rt/asm/*.s` and `rt/ir/di.ll`, with the symbols
   renamed `<sym>__sn`, and its calls renamed in the assembly. (The alternative, far libcalls in the backend
   plus far runtime routines, is bigger work.)
3. A linker script with one `SEGMENT = AUTO` region per code segment, a start that does `call main@seg:main`,
   and HAL routines in assembly that return with `retf`.
4. **One linker change** (in the repository, tested): `i8086-ld --exe` refused more than 121 segment
   relocations because the MZ header was one fixed 512-byte sector; a medium-model program has one relocation
   per far call (178 here, a few thousand for BASIC). The header is now as many whole sectors as the table
   needs (`llvm-i8086/asm/src/output.rs`, test `a_relocation_table_longer_than_one_sector_gets_a_longer_header`
   in `asm/tests/exe.rs`; all 37 i8086-ld tests and clippy pass; README updated).

Limits of the proof (to handle in M0): no function pointers or `dyn` in the program (the interpreter
dispatches with `match`); calls inside one segment are far too (an optimisation could keep them near);
`panic` must not pull in `core::fmt` (`panic_immediate_abort` or a handler that does not format).

## 2. Proposed architecture

- **BASIC.COM is a medium-model `.EXE`** (MZ signature, named BASIC.COM: DOS 2.00 decides by the signature;
  spec D1). Segment 0 = the PSP's program segment: start code, the most-called interpreter code, Rust's
  rodata/data/bss and a 4 KiB stack (DS = SS = segment 0 for all Rust code). Code segments 1-2 (maybe 3):
  the rest, packed by `ir_medium.py`.
- **BASIC's data segment**: a separate 64 KiB DOS block with the original's observable layout (spec §3):
  TXTTAB at 0CB6h, program text, variables, arrays, strings, file buffers; reached from Rust with the far
  intrinsics (`peek`, `poke`, `peek16`, `poke16`) and a few assembly bulk routines (far `memmove`, compare,
  line search) for speed. Bytes free and FRE come out as the original's because the arithmetic is the same.
- **Interpreter state** (FAC, the text pointer, FOR/GOSUB stack, file table, screen editor state) are Rust
  statics in segment 0 (spec D2). The BASIC stack used for FOR/GOSUB/WHILE lives in the data segment below
  MEMSIZ like the original, so `Out of memory` happens at the same depth.
- **HAL** (`src/basic.s`, all `retf`): INT 21h with a register block (DS:DX buffers in either segment),
  INT 10h/16h, video memory writes, the INT 1Bh/23h/24h handlers (set a flag), SHELL's EXEC with the stack
  switch, `SYSTEM`. A Rust trait `Machine` in `lib.rs` hides them so a host model can run the cases.
- **Core crate** (`lib.rs`, `no_std`): modules `mbf` (single/double formats and arithmetic, ported from GW
  MATH1/MATH2), `fin`, `fout` (with PRINT USING), `trans` (SIN..RND), `token` (tables of spec §4, crunch,
  list), `text` (lines in the data segment), `vars` (PTRGET-like search, arrays), `strings` (space and GC),
  `eval`, `stmt` (dispatch by token), `screen` (editor and key line), `files` (devices, DOS handles, buffers),
  `progio` (SAVE/LOAD/MERGE/CHAIN, protected encoding), `errors`.
- **Tests**: host replay of every case (a model of the 25×80 screen, the keyboard script and a FAT disk via
  `pcdisk`), MartyPC and the Rust emulator for the four-way matrix (spec/software-lifecycle.md), unit tests of
  MBF against tables generated from the oracle (FOUT/FIN of many values captured with a BASIC program).

## 3. Milestones

| M | Content | Cases it must pass | Size (KB) |
|---|---|---|---|
| (status) | **M0 and M1 done 2026-11-10**: `scripts/build.sh` → `build/basic.com` (22,692 bytes; segment 0: 4,390 bytes of code, 3,024 of rodata, 4,102 of .bss and stack; far segment 1: 9,635 bytes of code — `MEDIUM_CAP0=0x1000` keeps segment 0 small on purpose so the far route is exercised from the start; all in one segment it was 22,689 bytes); M1's cases and b21 identical on the host model, on the Rust emulator and on MartyPC; the original identical on the Rust emulator on all 31 cases | | |
| M0 | Medium-model build moved into the toolchain (after approval, §5): `build_rust_medium.sh`, the pass, per-segment runtime, linker script template; a hello `BASIC.COM` that prints the banner with `Bytes free` from a real 64 KiB data segment and returns on `system` | — (a smoke run on MartyPC and the Rust emulator) | 3 |
| M1 | Screen output, key line, line input (Enter, backspace), crunch/list, line storage, direct mode, `ERROR n`, error messages, NEW/LIST/RUN skeleton, WIDTH, KEY | b29, b30, b14-b18, b31 | 25 |
| M2 | Evaluator, MBF single/double, FIN/FOUT, PRINT USING, transcendentals, RND, variables/arrays/DEF FN, control flow, strings, ON ERROR, `basic FILE` (ASCII LOAD) | b01, b02, b03, b09, b10, b11, b12, b20, b27, b28 | 60 |
| M3 | Files and devices, OPEN/CLOSE/PRINT#/INPUT#/WRITE#/LINE INPUT#/FIELD/GET/PUT/LSET/RSET/LOC/LOF/EOF, SAVE/LOAD/MERGE/CHAIN/COMMON in all formats, FILES/KILL/NAME | b04, b05, b06, b07, b08, b13, b19, b25, b26 | 80 |
| M4 | The full screen editor (control keys, logical lines), INPUT/LINE INPUT/INPUT$, AUTO/RENUM/DELETE/EDIT/TRON/CONT, LOCATE/CSRLIN/POS/SCREEN() | b21, b22, b23, b24 | 88 |
| M5 | The rest: graphics, SOUND/BEEP, LPT/COM, CALL/USR/BLOAD/BSAVE, CLEAR, /F /S, SHELL; new cases with framebuffer hashes for graphics; the four-way matrix on all cases; report | all, four ways | 95-110 |

Each milestone ends with the host replay and MartyPC runs of its cases green and the earlier ones still green.

## 4. Risks

| Risk | Mitigation |
|---|---|
| Code larger than estimated | more code segments cost nothing but packing; the data in segment 0 (rodata, stack) must stay under ~40 KB: keep tables compact |
| Bit-exact MBF arithmetic and number printing | port GW-BASIC's algorithms (same code base as IBM's) instruction by instruction; capture extra oracle tables (a BASIC program printing FOUT of thousands of values to a file, compared byte for byte) |
| Speed (the 700 M-cycle budget per case) | inline far access, bulk assembly helpers for line search and string moves; measure cycles per case on MartyPC from M2 |
| No function pointers in the medium model | design with `match` dispatch; the pass refuses violations at build time; a thunk-table extension if ever needed |
| `panic` pulling in `core::fmt` | abort without formatting; check the IR for `fmt` |
| Ctrl-Break and SHELL cannot be driven by the harness | implement as GW; document as untested; SHELL case left out (as for b01-b09) |
| Hidden ROM behaviour the cases do not show | more cases per milestone (the harness takes ~12 s per case) |

## 5. Decisions (asked 2026-11-09, answered 2026-11-10: all recommendations accepted)

Answers: (1) the route moved into `llvm-i8086/scripts/` and `rt/medium/` with the test `tests/medium/`;
(2) the i8086-ld header change stays; (3) BASIC.COM ships as a multi-segment `.EXE` (spec D1); (4) the simple
layout, recorded as spec D4; (5) only what programs are documented to rely on is reproduced in 0000h-0CB5h
(spec D2); (6) graphics and sound wait for M5 with screen-image capture.

The questions as asked:

1. **Toolchain extension**: approve moving `medium_model/scripts/{ir_medium.py, seg_rt.py}` into
   `llvm-i8086/scripts/` as the supported medium-model route (with a run test in
   `system/ibm_pc/emulator/tests/`), or keep it title-local in `basic/scripts/`. The i8086-ld header change
   is already in the working tree (small, tested) for review.
2. **D1**: a multi-segment `.EXE` named BASIC.COM (instead of a `.COM`); no alternative fits.
3. **M-1** (io.md): load low (code then data) or place the data segment at exactly PSP+403h like the
   original (code above it); the cases cannot tell, absolute addresses differ.
4. **Scope of D2**: keep only the observable offsets of the data segment (TXTTAB, VARTAB, text, variables) or
   mirror more of the original's 0000h-0CB5h work area for programs that PEEK it.
5. Graphics and sound need framebuffer/audio oracles that the current text transcripts do not have; agree to
   add framebuffer-hash cases in M5 (the harness supports text only today).
