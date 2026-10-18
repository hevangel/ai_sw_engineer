# Busicom 141-PF firmware — report

Status at milestone M1 (2026-10-18).

| Check | Result |
|---|---|
| Host replay vs original ROM (recovered-ROM profile) | **7/42** examples: 1-1, 1-2, 1-3, 1-4, 14-1, 14-2, 14-3 |
| BCD unit tests | 3/3 |
| `cargo fmt --check`, `clippy -D warnings` | clean |
| Target image size vs 1280-byte ROM | n/a — `llvm-i4004` not started |
| Primary simulator (ai_hw_engineer RTL) | n/a |
| Secondary emulator (`external/busicom`) | n/a |

Toolchain: `ai-sw-engineer` image (Rust 1.99.1 host).

## Known gaps

- Not implemented: `* / % EX`, constants, chain and power calculations, `CM RM M+ M- M=+ M=-`, `SQRT`, FL result precision; their 35 examples fail by design.
- Assumptions A1–A4 in `spec/spec.md` are unverified against the original ROM; none is exercised by the 7 passing examples except A2 (1-2).
- The keyboard buffer, printer drum sequencing and lamp drivers (M3) do not exist; the core prints whole rows.
