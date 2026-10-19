# Rust → i4004 IR path

The pinned `rustc` (nightly-2026-10-15, LLVM 23.1.3) has **no i4004 target**, so
it cannot emit i4004 LLVM IR directly. This directory holds the glue that gets
the Busicom firmware crate to i4004-shaped IR anyway, plus the Rust target JSON.

| File | Purpose |
|---|---|
| `i4004-none.json` | Rust target spec; its `data-layout` matches `llvm-i4004/spec/abi.md §1` character-for-character (for the day rustc can target i4004 directly). |
| `retarget_ir.py` | Rewrites an `avr-none` IR module's triple + datalayout to i4004, with hard refusals for anything target-specific it cannot map. |
| `codegen_worklist.md` | Generated: the ranked list of operations our `llc` cannot yet select for the whole firmware. |

## Why `avr-none` is the stepping stone

We compile the firmware for the **`avr-none`** target — the closest *existing*
LLVM target whose data layout is compatible with i4004's — then textually
retarget the emitted IR. The two layouts:

```
avr-none : e-P1-p:16:8-i8:8-i16:8-i32:8-i64:8-f32:8-f64:8-n8:16-a:8
i4004    : e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8
```

Compatible where it matters for this firmware:

| Property | avr-none | i4004 | OK? |
|---|---|---|---|
| Endianness | little (`e`) | little (`e`) | ✅ identical |
| Data pointer | `p:16:8` (16-bit, byte-aligned) | `p:16:8` | ✅ identical |
| `i8` / `i16` alignment | 8-bit | 8-bit | ✅ identical |
| Native int set | `n8:16` | `n8` | ✅ i4004 is a subset; nothing in the IR relies on `i16` being "native" for correctness |
| Aggregate alignment | `a:8` | `a:8` | ✅ identical |

Differences the retarget handles or tolerates:

- **Code address space.** AVR is Harvard: all code lives in program space, so
  every `define`/`call`/intrinsic in the AVR IR carries `addrspace(1)` (the
  `P1` clause makes program space the default for code). i4004 is also Harvard
  (code in ROM) but its datalayout leaves the default space `0` and reserves
  `p1`/`p2` for ROM-constant / RAM-status *data* pointers. `retarget_ir.py`
  **drops `addrspace(1)` from code** (556 occurrences in the current build);
  this is behaviour-preserving because the firmware IR has **no data pointer**
  in a non-default address space (the script refuses the module if it finds
  one — see below).
- **`m:e` (ELF mangling).** i4004 emits ELF (`abi.md §5`); AVR IR omits the
  mangling clause. Added by the rewrite. No effect on the IR body.
- **Extra `i32/i64/f32/f64` datalayout entries** in the AVR string are dropped;
  the firmware uses no floating point (verified — the script refuses FP), and
  `i32`/`i64` keep the generic default layout, which is what the backend
  legalizes against.

## The build

`system/busicom_141pf/firmware/scripts/build_target_ir.sh`:

1. `cargo +nightly rustc --bin firmware --target avr-none -Z build-std=core`
   `-Z build-std-features=compiler-builtins-mem` with
   `RUSTFLAGS="-C target-cpu=atmega328p -C opt-level=z -C panic=abort -C lto=fat
   -C embed-bitcode=yes -C linker=/bin/true"`, `--emit=llvm-ir`.
   - `build-std=core` compiles `core` for avr-none alongside the crate (the
     pinned rustc ships no prebuilt avr `core`).
   - **fat LTO** pulls the firmware crate and the needed bits of `core` into a
     single whole-program module — one `.ll`, not a per-crate pile.
   - `panic=abort` + `opt-level=z` keep the panic/formatting machinery out (the
     emitted IR has **no `core::fmt`, no panic strings, no `float`/`double`,
     no `i128`** — confirmed by grep).
   - `linker=/bin/true` lets codegen finish (which is where `--emit=llvm-ir`
     happens) without a real `avr-gcc`; we never want a linked AVR binary.
2. `retarget_ir.py` rewrites the header to i4004 → `build/firmware_i4004.ll`.

## `retarget_ir.py` — exact mapping and refusals

Rewrites (behaviour-preserving):

- `target triple` → `i4004-none`
- `target datalayout` → the i4004 string above
- strips `addrspace(1)` from code (Harvard program space → i4004 default space)

**Refuses** (exit 2, no output) — a module that is target-specific in a way we
must not silently fake:

- any `llvm.avr.*` intrinsic, `__flash`/`progmem` token, or `addrspacecast`
- a `float`/`double`/`half`/`fp128` type (the 4004 has no FP unit)
- an `i128` (not legalizable on a 4-bit machine)
- a **data** pointer in a non-default address space (`ptr addrspace(N)`, N≠0, on
  a load/store/GEP/alloca) — a real memory-model difference, not just a code
  annotation

`i32`/`i64` are **reported, not refused**: they are legal IR the backend must
widen-legalize (and they are the costly part of a 4-bit target, so they belong
in the codegen work list, not in a hard stop).

## Status

- ✅ Whole-program `avr-none` IR builds (fat LTO + build-std), 40 functions,
  only the four `__bus_*` board functions left external.
- ✅ Retargets to i4004 cleanly; the refusal guards are tested against synthetic
  FP / i128 / avr-intrinsic / data-addrspace inputs.
- ⏳ Our `llc -mtriple=i4004` selects **1 of 40** functions so far; the rest need
  the codegen work in `codegen_worklist.md` (the codegen agent's backlog).
- ⏳ `i4004-none.json` is the target spec for the day rustc can target i4004
  directly (building rustc against our LLVM, rust-mos style); not needed for the
  avr-stepping-stone path and **not** exercised yet.
