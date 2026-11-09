# Medium-model proof (issue #301, phase 1)

Evidence for [../implementation_plan.md](../implementation_plan.md) §1: a Rust program built through the
llvm-i8086 msp430 route with its code in three 64 KiB segments (every function far), a separate 64 KiB far
data segment, and per-segment runtime copies, run under PC DOS 2.00.

| Path | Contents |
|---|---|
| `crate/` | the proof program (`src/logic.rs` shared with the host reference in `host/`; `src/pad.rs` is generated) |
| `scripts/ir_medium.py` | the IR pass: far intrinsics → `addrspace(1)` loads/stores; `cc 1000` everywhere and segment packing |
| `scripts/seg_rt.py` | per-segment renamed copies of `llvm-i8086/rt` for the libcalls of far segments |
| `scripts/start_far.s`, `hal_far.s`, `hal_near.s`, `med.ld` | start, HAL and linker script of the two builds |
| `scripts/gen_pad.py` | the ~95 KB of padding functions |
| `scripts/build_proof.sh <out>` | builds `tiny.com` and `medium.exe` and prints the host's expected text |

Run each on MartyPC with `capture_dos.sh` and a two-line `cases.py` (`m_tiny - tiny`, `m_medium a:system360
medium`; the 120 KB `.EXE` needs the 360 KiB disk) and `PROGRAM=<file> NAME=TINY.COM|MEDIUM.EXE`. Result on
2026-11-09: both screens show the host's text.
