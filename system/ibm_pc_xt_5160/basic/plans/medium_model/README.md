# Medium-model proof (issue #301, phase 1)

The proof that a Rust program can have its code in several 64 KiB segments, done in phase 1 here, now lives in the
toolchain as the supported route (2026-11-10):

- the route: `llvm-i8086/scripts/build_rust_medium.sh` (with `ir_medium.py`, `seg_rt.py`, `medium_ld.py`) and
  `llvm-i8086/rt/medium/` (`start_far.s`, `far.s`), documented in `llvm-i8086/spec/abi.md` D10 and
  `llvm-i8086/rust/README.md` "Medium model";
- the proof program and its test: `llvm-i8086/tests/medium/` (`proof/`, about 100 KB of code in three segments,
  run under PC DOS 2.00 on the Rust emulator against the host by `tests/dos_run.rs`).

Phase 1's result, kept for the record: the 120,009-byte `medium.exe` (three code segments, 178 segment
relocations) ran under PC DOS 2.00 on MartyPC's IBM 5160 and printed the host's text, as did the one-segment
`tiny.com` of the same source. BASIC.COM is built by the same route (`../../scripts/build.sh`).
