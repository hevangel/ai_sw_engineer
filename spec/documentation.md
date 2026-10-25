# Documentation and design docs

Read this before finishing any task that changes behavior, layout, scripts, an ABI or a memory/I/O map, and before writing a README.

- Markdown everywhere. Maintain `system/README.md` (system index) and `llvm-*/README.md`.
- Every software README states the original's year, authors, sources and the current status. Historical dates are sourced; uncertainty is stated, not guessed.
- **Design docs stay current.** Status docs (README status, `report/`, `size_history.md`, plan tables) record results. Design docs record how the thing works, and they go stale silently unless someone keeps them current. The design docs are:
  - **Backend:** `spec/codegen.md`, `spec/abi.md`.
  - **Title:** `spec/spec.md` (behavior and the assumption ledger), `spec/io.md` (I/O contract and drivers), `plans/testplan.md`, and the README layout table and run commands.
  - **Platform:** `memory_map.md`, `io_map.md`.
  - **System emulator:** `emulator/spec/spec.md` (machine contract, chip cores and pins, unsupported hardware, assumption ledger), `emulator/plans/testplan.md`, and the emulator README run commands.

  A change that alters what one of them says updates it in the same change: a new data layout or RAM map, a driver moved to assembly, a new test level, a new script, a calling-convention change.
- Every `ASSUMPTION` in code cites a ledger ID (`ASSUMPTION (spec M3-K2): …`), and that ID exists in `spec/spec.md` with the same meaning. A new assumption gets a new ID; an ID is never reused for something else.
- A superseded design is marked frozen or historical with a date, not deleted. If a design doc disagrees with the code, the doc is the defect: fix it when you find it, even outside your task.
- Before finishing a task, check the design docs for the area you changed. A status update alone (README line, report) does not count as updating the docs.
- When you change the rules in this `spec/` folder, keep the index in `AGENTS.md` and `spec/README.md` in step, and update the human-facing `README.md` if the change is visible to a reader of the project.
