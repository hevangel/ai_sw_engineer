# Size probe (issue #301, phase 1)

A representative slice of the interpreter — tokenizer and lister with the keyword table, MBF single and
double arithmetic, FIN, FOUT, a PRINT USING field, a small evaluator (1,139 lines) — built only to measure
code size with the title profile (`opt-level = "z"`, fat LTO). It is not the rewrite and is not run.

    bash llvm-i8086/scripts/build_rust.sh <this dir> slice <out>   # then link with link_com.ld, crt0_com.o
                                                                    # and a probe_in/probe_out stub

Result 2026-11-09: 17,102 bytes of code, 1,600 of rodata ([../implementation_plan.md](../implementation_plan.md) §1).
