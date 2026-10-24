# monitor implementation plan

1. Collect sources; record provenance of the original.
2. Capture oracle transcripts from the original on the simulator.
3. Write `spec/spec.md` and `spec/io.md`.
4. Implement the core in `src/lib.rs`; host replay passes.
5. Emit LLVM IR; build the target image with `llvm-i8008`; record size vs budget.
6. Run on the simulator; compare against the same transcripts.
7. Write `report/final_report.md`.

## Design decisions

TODO (data widths, state layout, what is deliberately not reproduced).
