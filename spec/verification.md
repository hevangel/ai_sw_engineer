# Verification before PR

Read this before opening or updating a PR, or when deciding whether a task is finished.

- [ ] Host: `cargo test` passes, oracle transcripts replay exactly
- [ ] System emulator changes: host tests, format and clippy pass; original-image replay matches committed oracle output (or the gap is documented), with commands and dependency/core provenance recorded
- [ ] `cargo fmt --check`, `cargo clippy -D warnings`
- [ ] Target image builds; size reported against the ROM/RAM budget
- [ ] Target image run on a simulator and compared against the oracle (or the gap is documented)
- [ ] Backend changes: lit + MC tests pass
- [ ] Backend changes: `spec/codegen.md` and `spec/abi.md` match the code (design changes documented in the same PR)
- [ ] Size scoreboard: a title whose target image is not yet signed off adds a row to `report/size_history.md` (date, commit, `.text`, image, largest function, call/libcall counts, host and target status) for every landing that touches it or its backend, measured the same way as the previous rows
- [ ] README / report updated
- [ ] Design docs for every touched area match the code (title `spec/`, `testplan.md`, README layout; platform `memory_map.md`/`io_map.md`; emulator `spec/spec.md`/`testplan.md`); every `ASSUMPTION` ID in the diff exists in `spec/spec.md`

See [documentation.md](documentation.md) for the design-doc rules and [git-workflow.md](git-workflow.md) for commit, branch and PR format.
