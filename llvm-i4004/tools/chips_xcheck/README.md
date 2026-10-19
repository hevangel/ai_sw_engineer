# chips_xcheck — differential disassembler cross-check

A **test tool**, not a shipped component. It is not part of any target crate and
is never published. It proves the `llvm-i4004` disassembler is correct by
EXECUTING every 4004 encoding on an independent emulator and checking that our
disassembly predicts the real effect.

## What it does

For every 4004 encoding:

1. disassembles the bytes with OUR `llvm-mc --disassemble` (mnemonic + operands);
2. predicts the machine effect from THAT disassembly using MCS-4 manual
   semantics (`../../spec/isa.md`), parameterised by the operands we printed;
3. **executes the same bytes on `external/chips`** (its real `mcs4::Board`) and
   recovers acc / carry / all 16 registers / RAM & ROM port & character writes
   through the chip's own instructions;
4. asserts prediction == observation. Control transfers use a landing harness.

The oracle is chips' actual execution, never a hand-written opcode table. A
wrong operand field or a swapped mnemonic is caught (see mutation tests).

## external/chips licence

`external/chips` carries **no stated package licence of its own**. This tool
therefore depends on it **by path, at build time** (`Cargo.toml` `chips = { path
= "../../../external/chips" }`) and only reads and runs it. No chips source is
copied into this repo.

## Run

Inside the `ai-sw-engineer` container (needs the i4004 `llvm-mc`):

```sh
# via the repo driver (also runs the executor self-test first):
LLVM_MC=/llvm-build/llvm-obj/bin/llvm-mc ../../scripts/crosscheck_disasm.py

# or directly:
LLVM_MC=/llvm-build/llvm-obj/bin/llvm-mc cargo run            # full cross-check
cargo run -- --smoke                                          # executor self-test only
```

It is wired into `../../scripts/run_tests.sh`.

## Mutation sanity

The check is proven able to fail:

```sh
MUTATE=swap_acc  cargo run   # our printer emits iac<->dac swapped -> 2 findings
MUTATE=reg_off1  cargo run   # every register operand off by one   -> findings across sweeps
```

## Layout

| File | Role |
|---|---|
| `src/exec.rs` | drives chips' `mcs4::Board`; readout harness recovers hidden CPU state; landing harness for branches |
| `src/predict.rs` | parses our disasm; per-mnemonic expected-effect model from the MCS-4 manual |
| `src/main.rs` | the checker loop, field sweeps, branch checks, mutation hook, `--smoke` |

## Known limitation

Only the chips half is executed. The second oracle (the `ai_hw_engineer`
`design/intel_4004` RTL) is not run here because no Verilog simulator is
available in the container or on the host. See the report's "Differential
cross-check" section.
