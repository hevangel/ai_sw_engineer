# BASIC oracle transcripts

`transcripts/<case>.txt`: the 25 text rows at the end of each case of [../scripts/cases.py](../scripts/cases.py),
run by the originals (BASIC.COM D2.00 on Cassette BASIC C1.10, [../original/provenance.md](../original/provenance.md))
on MartyPC's IBM 5160 with `system/ibm_pc_xt_5160/scripts/capture_dos.sh`.

- b01-b09: the first set (programs, files, an interactive session, direct-mode errors).
- b10-b31 (issue #301 phase 1, 2026-11-09): number formatting (b10), PRINT USING (b11), strings and FRE (b12),
  trapped errors (b13), every error message (b14-b18, b31), tokenized and protected SAVE bytes (b19), LIST of
  crunched lines (b20), the screen editor's control keys (b21), AUTO/RENUM/TRON/STOP/CONT/DELETE (b22), INPUT
  and `?Redo from start` (b23), LOCATE/CSRLIN/POS/SCREEN()/KEY LIST (b24), CHAIN MERGE and COMMON (b25), file
  modes and file errors (b26), variable types and VARPTR (b27), the data segment's layout (b28), the
  function-key line at 80 and 40 columns (b29, b30; these end in BASIC, without SYSTEM).

Re-running b10, b19, b22 and b28 gave byte-identical transcripts (deterministic). Never re-capture b01-b09.
