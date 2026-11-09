# Reference: Microsoft GW-BASIC source

| | |
|---|---|
| Repository | [microsoft/GW-BASIC](https://github.com/microsoft/GW-BASIC) |
| Commit | `edf82c2ebf6bfe099c2054e0ae125c3efe5769c4` (2020-05-21, "Added announcement blog link") |
| License | MIT |
| What it is | the 8086 assembly source of GW-BASIC as of 1983 (`IBMRES.ASM`'s translation header: 10-Feb-83), released by Microsoft in 2020; not buildable as published |

The source is **not copied** into this repository; clone it at that commit when needed. It is the same code
base as IBM's BASIC: `IBMRES.ASM` builds "GW-BASICs with IBM PC compatible reserved word tokens", and its
tables agree with the tokens decoded from the IBM ROM and BASIC.COM ([../spec/spec.md](../spec/spec.md) §4);
the protected-SAVE encoding of `GIODSK.ASM` (`PENCOD`) reproduces the oracle's encrypted file byte for byte
(spec §11). It is a reference only: the IBM binaries are the oracle, and every place where the source
disagrees with them is listed in the spec's ledger.

Files the rewrite follows (routine names are used in the spec):

| File | For |
|---|---|
| `MATH1.ASM`, `MATH2.ASM` | MBF arithmetic, `$FIN`, `$FOUT`, PRINT USING number formatting, SIN/LOG/EXP/SQR/`^`/RND |
| `BIPRTU.ASM` | the PRINT USING driver |
| `GWMAIN.ASM`, `GWLIST.ASM` | crunch, list, statement dispatch, FOR/GOSUB, errors, INPUT/READ |
| `GWEVAL.ASM`, `BIPTRG.ASM` | expression evaluation, variables and arrays |
| `BISTRS.ASM` | strings and garbage collection |
| `BIMISC.ASM`, `FIVEO.ASM` | NEW/CLEAR/CONT/TRON/SWAP/ERASE, WHILE/WEND, CHAIN, WRITE |
| `SCNDRV.ASM`, `SCNEDT.ASM` | the screen editor |
| `DSKCOM.ASM`, `GIODSK.ASM`, `GIO86.ASM` | files, SAVE/LOAD, FIELD/GET/PUT, protected programs |
| `GWINIT.ASM`, `GWRAM.ASM`, `GWDATA.ASM` | start, memory layout, the data segment's variables |
| `IBMRES.ASM` | the reserved-word tables |
