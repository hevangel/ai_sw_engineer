# Plan: IBM PC 5150 compiler profile (issue #58)

State on 2026-10-27: the shared backend and rustc target exist and are verified ([../../../../llvm-i8086/report/final_report.md](../../../../llvm-i8086/report/final_report.md)); the platform runtime, linker scripts and HAL exist in [../../platform](../../platform). What the issue still asks for, and where it lands:

| Acceptance item | Work | Where |
|---|---|---|
| Audit and pin | write down what upstream lacks and the exact versions | [../spec/audit.md](../spec/audit.md), `scripts/versions.sh` |
| Machine/CPU/ISA/ABI specification | one document for this computer | [../spec/profile.md](../spec/profile.md) |
| Target specification | `i8086-unknown-none` in rustc | done; documented in the profile |
| Startup, memory map, linker, vectors, HAL | exist; document as the contract | profile + `platform/` |
| Build/link/package commands with size checks | one script builds every image format from Rust source and checks sizes | `scripts/build_profile.sh` |
| Smoke programs and an independent decoder | decode every image with GNU objdump and Capstone; check entry, relocations, placement, forbidden instructions | `scripts/check_images.py` |
| Execution on independent models | RTL (existing transcripts), MartyPC (`martypc_boot_images.sh`), Rust emulator tests; expected output recorded | `scripts/run_tests.sh` |
| Regression tests and report | `run_tests.sh` and a report with versions, commands, hashes, results, limits | [../report/report.md](../report/report.md) |

Non-goals: the application or OS rewrite (ai_engineer#81), floppy formats other than the 160 KiB single-sided disk, 256 KiB RAM configurations, the XT 5160 profile (ai_sw_engineer#59).
