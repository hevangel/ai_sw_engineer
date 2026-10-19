# Escaped defect: the Rust Busicom firmware is ~11x too big for the Intel 4004

Date: 2026-10-19
Area: `system/busicom_141pf/firmware` + `llvm-i4004`
Branch: `system/busicom_141pf-on-4004`

## What escaped

The project's end-to-end goal is: compile the Rust Busicom 141-PF firmware with
our own toolchain (rustc avr-none IR -> retarget -> llc -mtriple=i4004 ->
llvm-mc -> i4004_pack.py) into a ROM image and run it on the 4004 (chips, then
the ai_hw_engineer RTL) reproducing the original printed tape. All the
component milestones were driven to green over several sessions — MC layer,
disassembler, CodeGen (43/43 functions now lower), the full libi4004rt runtime
(i8 + i16 arithmetic/shift/bitwise + mem helpers, 1351/1351 chips execution
tests), crt0, HAL, the image packer, and the xl board model. The implicit
assumption behind every one of those was that, once the backend was complete,
the pieces would link into a 4 KB image.

They do not. When the whole firmware was first compiled to a relocatable object
and handed to the linker (this session), the image could not be built:

- Total firmware `.text` = **45,714 bytes** across 43 functions
  (`llc -function-sections -filetype=obj`), against the 4004's **4096-byte**
  architectural ROM maximum (16 x 4001). ~**11.2x** over.
- Largest single function `target::run` = **8,570 bytes** — alone > 2x the
  entire address space. `bcd::Num::divide` 5,138; `sqrt_at` 4,283; printer
  `print` 3,712; `Num::mul` 2,664. Eleven functions exceed 1 KB.

A second, independent encodability problem surfaced at the same point: the
4004's `JCN`/`ISZ`/`FIN` branch only within the current 256-byte ROM page, so
any function > 256 bytes needs far-conditional-branch relaxation
(`JCN !cc,$+n ; JUN far`) before its own control flow is encodable at all. Every
function but the smallest exceeds 256 bytes.

## Why the title's own tests did not catch it

The firmware's host tests (`cargo test`, 42/42 exact) and all the libi4004rt /
codegen execution tests run per-function or on tiny linked stubs, never on the
whole linked image — because `build_image.sh` was a stub until this session and
nothing ever asked the linker to place all 43 functions together under the
4 KB cap. The size ceiling is a property of the *whole program on the target*,
which no component test exercises. The oracle (the original ROM tape) is about
behaviour, not size, so it could never flag it either.

## Root cause

A general-purpose SSA backend lowering `no_std` Rust — even size-tuned
(`opt-level=z`, LTO, `panic=abort`) — emits on the order of 10x more code than
the original hand-written MCS-4 ROM (1,280 bytes) for the same behaviour: i16
and nibble-serial BCD operations expand to long ACC/CY instruction chains, each
`bcd::Num` method is a full out-of-line function with call/spill overhead, and
the 4004 has no reg-reg ALU so every operation is LD/ADD/XCH sequences. 4 KB of
4004 ROM cannot hold 45 KB of such code.

## Options (a decision is required before more backend work)

(a) A dramatically more compact code model. Not credible for an ~11x gap from
    instruction selection / regalloc tuning alone.
(b) Rewrite the firmware small enough that its *compiled* image fits 4 KB — a
    redesign, and at odds with the "compile the existing Rust firmware" premise.
(c) Accept the stock hand-written 1,280-byte ROM as the only image that fits the
    hardware, and treat the Rust -> 4004 path as validated at the
    component/function level (which it now is: 43/43 lower, runtime 1351/1351 on
    chips) rather than as a whole-image run.

## Prevention

Add a whole-program SIZE gate early: as soon as a backend can emit objects for
the real title IR, link-and-measure against the ROM budget in CI, well before
investing in linker/relaxation polish. A target-size sanity check (sum of
per-function .text vs ROM max) belongs in `gen_worklist.py`'s report.
