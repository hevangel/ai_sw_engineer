# System and software templates

- `system_README.md` → `system/<system>/README.md`
- `platform/` → `system/<system>/platform/`
- `software/` → `system/<system>/<software>/`

Placeholders `[SYSTEM]`, `[SOFTWARE]`, `[CHIP]` are replaced by `scripts/new_system.sh` and `scripts/new_software.sh`. The software template is a working Rust crate (placeholder echo core + oracle replay test) so a fresh title passes `cargo test` before any real work.

## Script contracts

| Script | Owner | Contract |
|---|---|---|
| `<software>/scripts/build.sh` | title | host build, host IR, then calls the backend hook |
| `llvm-<chip>/scripts/build_image.sh <title_dir> <out_dir> <link.ld>` | backend | writes `<out_dir>/<software>.{ll,s,elf,bin,map}` |
| `platform/scripts/run_image.sh <primary\|secondary> <image> <transcript>` | platform | runs the image on a simulator, prints `<` output lines |
| `<software>/scripts/compare.sh` | title | diffs simulator outputs with oracle transcripts |
