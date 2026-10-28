# Test plan — MC6800 Rust core

All tests run on the host: `cargo test --manifest-path
chip/motorola_6800/emulator/Cargo.toml`.

1. **Full-ISA vector replay** (`tests/vector_replay.rs::full_isa_vector_replay`)
   — replays `../oracle/isa`: the directed program covering all 197
   documented opcodes in every addressing mode plus interrupt/WAI/SWI/NMI
   scenarios. Compares (pc, a, b, ix, sp, cc) at every opcode-fetch
   boundary (1,948 records), fires the clock-keyed external IRQ/NMI
   triggers with the RTL testbench's exact engine semantics, applies the
   $E000 control port, and compares the final 64K memory against the
   oracle's gold image.
2. **Control-port visibility** (`tests/vector_replay.rs::control_port_write_visible_on_tick`)
   — the harness observes the core's real bus write of $AA to $E000.
3. **Third-party suite** (`tests/thirdparty.rs`) — all 50 assembled
   hakanrw/mc6800 programs free-run to their self-loop and the
   $2000-$20FF marker window must equal the ISS oracle's window (the same
   windows that gate the RTL in ai_hw_engineer).

The oracle data was produced by
`ai_hw_engineer/design/motorola_6800/scripts/make_isa_vectors.py` and
`assemble_thirdparty.py` (ISS execution of each image); neither the Rust
core nor its expected windows were derived from the core under test.
