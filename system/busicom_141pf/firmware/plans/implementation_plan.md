# Busicom 141-PF firmware — implementation plan

Goal: a Rust reimplementation of the 1971 calculator firmware that prints exactly what the original ROM prints for all 42 manual examples, compiled through LLVM to 4004 machine code and run as the ROM of the ai_hw_engineer board.

## Milestones

| | Scope | Exit criterion | Status |
|---|---|---|---|
| M0 | Title scaffold, oracle import, replay harness | `cargo test` runs all 42 examples and reports a score | ✅ done |
| M1 | BCD core; entry; `+ - ST = C CE SIGN`; DP alignment; rounding at total; overflow | examples 1-1…1-4, 14-1…14-3 pass | ✅ done |
| M2 | `* / % EX`, constants, chains, powers, mixed calc; memory keys; `M=+ M=-`; square root; FL precision | 42/42 on the host | ✅ done (42/42) |
| M3 | Machine drivers: keyboard scan via 4003, 8-key buffer, selectors, printer drum/hammer sequencing on TEST, lamps — tested against a board model on the host | same 42/42 through port-level simulation | ✅ done (42/42) |
| M4 | `llvm-i4004` backend sufficient for this crate (see `llvm-i4004/plans/`) | `build_image.sh` emits a 4001 ROM image | pending |
| M5 | Image on ai_hw_engineer RTL board and `external/busicom` | 42/42 on the RTL; size vs 1280 / 4096 bytes reported | pending |

## Design decisions

- **Digit-serial BCD, 16 nibbles per register** (`src/bcd.rs`), sign and decimal position kept beside the digits — the same representation the original keeps in 4002 registers. No binary integers wider than `u8` in the core, so the 4004 backend never needs 16/32-bit arithmetic for the calculator itself.
- Core separated from drivers so M1–M2 can be checked against the oracle on the host while the backend does not exist yet.
- Size is the dominant risk: 1280 bytes is hand-written-assembly density. The report will give size against 1280 bytes (original) and 4096 bytes (16 × 4001, the MCS-4 maximum); exceeding 1280 is expected at first and recorded, not hidden.
- The 3-level 4004 call stack means the backend will inline or use a RAM software stack; keep the core's call graph shallow (no recursion, few levels).
