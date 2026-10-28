# System and software templates

- `system_README.md` → `system/<system>/README.md`
- `platform/` → `system/<system>/platform/`
- `emulator/` → `system/<system>/emulator/` (host Rust whole-system emulator crate)
- `software/` → `system/<system>/<software>/`

Placeholders `[SYSTEM]`, `[SOFTWARE]`, `[CHIP]` are replaced by `scripts/new_system.sh` and `scripts/new_software.sh`. The software and emulator templates are working Rust crates (placeholder echo core + oracle replay test) so a fresh title or system passes `cargo test` before any real work. The emulator crate is a host `std` crate with a fixed library name (`system_emulator`); fill `emulator/spec/spec.md` first, then replace its placeholder core with the real CPU, memory and peripherals.

## Script contracts

| Script | Owner | Contract |
|---|---|---|
| `<software>/scripts/build.sh` | title | host build, host IR, then calls the backend hook |
| `llvm-<chip>/scripts/build_image.sh <title_dir> <out_dir> <link.ld>` | backend | writes `<out_dir>/<software>.{ll,s,elf,bin,map}` |
| `platform/scripts/run_image.sh <primary\|secondary\|emulator> <image> <transcript>` | platform | runs the image on a simulator (`emulator` = the `emulator/` CLI), prints `<` output lines |
| `<system>-emulator <image> <input>` (`emulator/` CLI) | emulator | replays inputs on an original or compiled image, prints observed output in oracle format, exits nonzero on invalid input |
| `<software>/scripts/compare.sh` | title | diffs simulator outputs with oracle transcripts |
