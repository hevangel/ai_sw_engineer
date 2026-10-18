# [SOFTWARE] ([SYSTEM])

| | |
|---|---|
| Original year | TODO (sourced) |
| Original authors | TODO |
| Machine | [SYSTEM] — [CHIP] |
| Status | scaffolded |

## History

TODO: what the program was, who wrote it, why it matters. Cite sources; state uncertainty.

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behavior spec and I/O contract |
| [original/](original/provenance.md) | provenance + SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts captured from the original |
| `src/` | `lib.rs` portable core, `hal.rs` I/O trait, `main.rs` target/host entry |
| `tests/` | oracle replay |
| [plans/](plans/implementation_plan.md) | implementation and [test plan](plans/testplan.md) |
| `scripts/` | build, host run, simulator run, compare, run_all |
| `bin/` | signed-off target images + `.sha256` |
| `build/` | generated IR/asm/objects/images (gitignored) |
| [report/](report/final_report.md) | final report |

## Run

```bash
./scripts/run_all.sh          # host test → LLVM IR → target image → simulator compare
./scripts/run_host.sh < in    # interactive host run
```
