# MIC 01 monitor oracle

The original ROM's bytes are unpublished, so there is no program to capture
from. The transcripts are written from R2E's *MICRAL N User's Manual*
(January 1974, chapter II): each expected output follows the manual's text,
and every format the manual leaves open is an assumption in
`../spec/spec.md` (MON-1..11), cited where it decides an output.

```text
oracle/transcripts/  — NN_name.txt; h* host only, t* target only (plans/testplan.md)
```

Format: `<` keys typed, `>` bytes printed (`\r \n \ \xNN` escapes),
`=` RAM preloads, `R` saved-register preloads, `?` memory checks, `#`
comments.
