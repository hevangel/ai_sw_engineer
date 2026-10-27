# Reverse engineering and the software title lifecycle

## Reverse engineering with REA

`external/rea/` ([morluto/rea](https://github.com/morluto/rea), MIT) is the standard tool for reverse engineering any original binary, ROM, disk image, firmware blob or application whose source is missing or unreliable. **Use it before reading disassembly by hand.** Read `external/rea/AGENTS.md` and the `reverse-engineer-anything` skill (`external/rea/.agents/skills/reverse-engineer-anything/SKILL.md`) first; `external/rea/docs/firmware-analysis.md` and `docs/ghidra-dos.md` cover raw ROM and DOS executables.

- Run it with `npx rea-agents setup` (registers the MCP server, with approval) or `npx -y rea-agents@latest <command>`; native analysis needs a bring-your-own Ghidra, IDA or Hopper provider. Do not install providers unasked. Never edit `external/rea/` in place.
- Keep REA's discipline: separate observations, derivations, inferences and unknowns. Commit the resulting Evidence summary (function map, memory map, call graph, strings, I/O register use, annotated listings you wrote) under `<software>/spec/` and cite the REA command/snapshot in `spec/spec.md`; never commit the original binary ([original-software](original-software.md)).
- REA output is an input to the specification, not the oracle. The oracle is still the original binary running on an emulator; REA explains *why* it behaves as it does.
- Old software that exists only as a binary (MS-DOS 3.3/5.0, QBasic, DOS Shell, Prince of Persia, classic ST games, ...) is reverse engineered with REA and rewritten in Rust; software with surviving source is rewritten from that source after REA confirms the shipped binary matches it.

## Software title lifecycle (every `[Software]` issue)

Each title in the website's software view has one issue in this repository, labelled `software`, `layer:*` (`os`, `driver`, `firmware`, `software`), `category:*` and `system:*`. Work the steps in order and tick them in the issue; each needs committed evidence:

1. **Original binary**: download the original binary/ROM/disk image; record URL, SHA-256 and license in `original/provenance.md` (the binary itself stays gitignored unless freely redistributable).
2. **Run the original** on both an independent open-source emulator (MAME, VICE, Stella, AppleWin, MartyPC, SIMH, ...; record name, version, configuration) and our Rust emulator (`system/<system>/emulator/`). Capture the oracle from the open-source run under `oracle/`; any disagreement between the two emulators is a finding.
3. **Source**: if source code exists, download it with provenance and license.
4. **Documents**: download every manual, datasheet, listing and article you can find; clean each into Markdown under `reference/` or `<software>/spec/docs/` with the source URL and SHA-256.
5. **Reverse engineer** the original with REA (the section above) and write `spec/spec.md` and `spec/io.md`.
6. **Rewrite in Rust** (`no_std` core + HAL, see [rust](rust.md)).
7. **Run the Rust rewrite** on both the open-source emulator and our Rust emulator, completing the four-way matrix: original or rewrite × open-source emulator or Rust emulator.
8. **Loop until all four combinations are identical.** Behavioral equivalence is proven by committed test cases that drive all four runs with the same timed, keyboard, mouse, joystick, paddle, disk or other inputs and compare the observable outputs (text, framebuffer hashes, audio samples, disk writes, cycle-timed events). Fix the rewrite, or our emulator when it is the outlier, until the matrix agrees. A title is not done until the whole suite passes in all four combinations; list any deliberate difference in the report.

The issue keeps `Status`, `Agent`, `Path` and `Start date` current per `../ai_engineer/spec/github_issues.md` and [github-issues](github-issues.md). Dependencies between titles (OS before application, predecessor before successor) are native blocked-by edges; do not start a title whose runtime prerequisite has no working oracle run yet unless the issue records why.

