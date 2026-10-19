# libi4004rt — the i4004 runtime library

Assembly (`llvm-mc` syntax, `llvm-i4004/spec/asm_syntax.md`) runtime the
`llvm-i4004` backend calls for operations the MCS-4 ISA cannot do in one
instruction (abi.md §6), plus the C-runtime startup (`crt0.s`). Everything here
is **execution-verified on the `external/chips` MCS-4 model** against a host
reference over many inputs — see `tests/`.

Assemble each `.s` with our `llvm-mc` (ELF, `e_machine=0x4004`); link with the
image packer (`llvm-i4004/asm/i4004_pack.py`). Each helper lives in its own
`.text.<name>` section so the packer's `--pad-pages` keeps it whole inside one
256-byte 4001 page — the in-helper `JCN` branches are page-local (isa.md §9) and
must not cross a page.

## Calling convention (recap, abi.md §4)

Arguments in pairs **P1, P2, P3** (`P<n>` = `R<2n>:R<2n+1>`, even reg = high
nibble). `i8` return in **P1**. `P0` is the reserved scratch pair; `P4–P6` are
callee-saved; `P7` is the software-stack pointer (SSP). Every helper here
touches only the caller-clobbered set (ACC, CY, P0–P3) unless noted, so a
caller's `P4–P6`/`P7` survive a runtime call. Call depth is ≤ 2 (a div/mod
wrapper → `__udivmod`), inside the 3-level hardware stack; no software stack is
used by the library itself.

### i4-return caveat (ABI refinement)

`BBL DDDD` loads `ACC ← DDDD` on return (isa.md §1), so a **callable** helper
cannot return an `i4` in ACC — the return instruction overwrites it. abi.md §4.2
names ACC as the `i4` return register; that holds for inlined/`JUN`-tail code,
not for a `JMS`/`BBL` callee. The out-of-line BCD helpers therefore return their
`i4` result in **P1 low nibble (R3)**. The backend that *inlines* a single BCD
instruction keeps the result in ACC as §4.2 says.

## The multi-nibble subtract rule (finding)

`SUB`/`SBM` compute `ACC + ~R + ~CY` (isa.md §7). For a **multi-nibble**
subtract the carry must be **complemented (`CMC`) between nibbles**: `CLC` once,
`SUB` the low nibble, then `CMC` before each higher nibble's `SUB`. Without the
`CMC` the borrow propagates with the wrong sense and every nibble above the
lowest is off by one. Verified on `chips` (`0x01-0x01=0x00`, `0x20-0x03=0x1D`,
`0x10-0x20` borrows, final `CY=1 ⇒ no borrow`). This is the idiom `__udivmod`
and the crt0/`hal` address compares use; the backend's `i16`/`i8` subtract
lowering must emit it too.

## Helpers

Sizes are bytes in ROM, measured from a full link (`--pad-pages`). The whole
library + crt0 + HAL is **697 bytes**, leaving ~583 of the 1280-byte budget for
firmware.

| Symbol | Lowered from | Contract | Size (B) |
|---|---|---|---|
| `__mulqi3` | `i8 * i8` | P1×P2 → P1 (low 8 bits). Shift-and-add, 8 iters. Clobbers P0–P3. | 38 |
| `__udivmod` | — (internal) | P1÷P2 → quotient P1, remainder P3. Restoring division, 8 iters. Div-by-zero UB. | 37 |
| `__udivqi3` | `u8 / u8` | P1÷P2 → P1. Wraps `__udivmod`. | 3 |
| `__umodqi3` | `u8 % u8` | P1%P2 → P1. Wraps `__udivmod`. | 7 |
| `__negqi2` | `-i8` | P1 → −P1 (two's complement). | 11 |
| `__ashlqi3` | `i8 << n` | val=P1, n=P2 → P1. n≥8 → 0. One-bit loop. | 21 |
| `__lshrqi3` | `u8 >> n` | logical right, zero-fill. n≥8 → 0. | 21 |
| `__ashrqi3` | `i8 >> n` | arithmetic right, sign-fill from bit 7. | 22 |
| `__andqi3` | `a & b` | P1&P2 → P1. Bit-serial (no ROM table). | 47 |
| `__orqi3` | `a \| b` | P1\|P2 → P1. Bit-serial. | 45 |
| `__xorqi3` | `a ^ b` | P1^P2 → P1. Bit-serial. | 47 |
| `__memset_ram` | `memset` | dst SRC-addr=P1, val=P2.lo, count(nibbles)=P3 → fills RAM; returns dst. | 34 |
| `__memcpy_ram` | `memcpy` | dst=P1, src=P2 (SRC addrs), count=P3. Forward. | 32 |
| `__memmove_ram` | `memmove` | overlap-safe (backward when dst>src). | 68 |
| `__bcd_adjust` | `DAA` | i4 in R3 (+CY) → R3. Decimal adjust. | 4 |
| `__key_decode` | `KBP` | one-hot→binary keyboard nibble; R3 → R3. | 4 |
| `__tens_comp` | `TCS` | ten's-complement step (9+CY); → R3. | 3 |
| `__dec` | `DAC` | R3−1 → R3; CY=0 ⇒ borrow. | 4 |
| `__push_byte` | spill | push `u8`=P1 at SSP (P7); SSP+=2. | 9 |
| `__pop_byte` | spill | SSP−=2; pop → P1. | 13 |
| `__push3` | sw-call | push 12-bit `{P2.lo:P1.hi:P1.lo}` as 3 nibbles; SSP+=3 (abi.md §4.4 return triple). | 13 |
| `__pop3` | sw-call | SSP−=3; pop the triple → P1.lo/P1.hi/P2.lo. | 19 |

### Units note for the mem helpers

They operate on **4002 characters = nibbles** (the addressable RAM unit), not
bytes, because the firmware's working registers are nibble arrays (`bcd::Num`).
A C `memcpy(n)` over `i8` byte-cells copies `2n` nibbles. They advance only the
low SRC nibble (char), so a single call stays within one 4002 register's 16
characters; a caller crossing a register/chip boundary splits the call
(documented limit).

### Signed division

`__divqi3`/`__modqi3` are **not** provided yet: the yardstick firmware is a BCD
magnitude+sign calculator and never emits signed binary division, so shipping an
unverified signed path would violate the "prove it" rule (AGENTS.md). Add them
(sign-magnitude wrappers around `__udivmod`, quotient sign = a^b, remainder sign
= a) when a title needs them, with the same exhaustive chips cross-check.

## crt0.s

Reset vector at ROM 0x000 (`.reset`). Order: select RAM bank 0 (`DCL`),
initialise SSP (`P7 ← __sstack_base`), clear `.bss`, copy `.data`'s ROM load
image to RAM (page-0 `FIN`, see below), `JMS main`, then a halt loop if main
returns. Size: 72 B.

`.data` copy is **page-local**: `FIN` indexes the current ROM page, so the copy
handles a `.data` load image on ROM page 0. A `.data` image on another page
needs a per-page `FIN` trampoline (abi.md §7). The Busicom firmware has **no**
`.data` (empty → the loop runs zero times), so this is exercised only by the
crt0 self-test. The `.bss` clear writes RAM via `SRC+WRM` and is not
page-limited.

Linker-defined symbols consumed by crt0 (emitted by the packer): `__sstack_base`
`__bss_start` `__bss_size` `__data_load` `__data_start` `__data_size`.

## Tests

`tests/run_rt_tests.sh [quick|full]` — builds a bare-CPU `chips` harness
(`tests/rt_test.rs`), then for each helper and input pair generates a stub
(`tests/gen_rt_test.py`), assembles stub+helper, links an image, runs it on
`chips`, and compares the read-back result nibbles to a host reference. `quick`
runs 624 cases (all green); `full` widens the arithmetic sweeps. The HAL glue
has its own suite under `system/busicom_141pf/platform/runtime/tests/`.

```
docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
  ai-sw-engineer:latest bash llvm-i4004/rt/tests/run_rt_tests.sh quick
```

The `chips` crate is a path dependency on the pinned `external/chips`; it is
run, never modified or copied (no license).
