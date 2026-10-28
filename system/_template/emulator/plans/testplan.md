# [SYSTEM] emulator test plan

| Level | What | Oracle | Command |
|---|---|---|---|
| Unit | chip wiring, memory map, error paths | datasheets / hand-assembled programs | `cargo test --manifest-path system/[SYSTEM]/emulator/Cargo.toml` |
| Replay | committed transcripts through the library | original on the primary oracle | `cargo test --test replay` |
| Original image | copyrighted image supplied by the user | committed original-image output | TODO: `<ENV_VAR>=/path/to/image cargo test --test replay -- --ignored` |
| Generated image | compiled title image through the CLI | same transcripts as the target run | TODO |
| Cross-check | RTL or another independent implementation | disagreement is a finding | TODO |

Hygiene: `cargo fmt -- --check` and `cargo clippy --all-targets -- -D warnings`.

Coverage goal: every peripheral and mode in `spec/spec.md` is exercised by at
least one test; every unsupported item is listed there.
