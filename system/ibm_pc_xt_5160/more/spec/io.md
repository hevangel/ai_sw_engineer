# MORE: I/O contract

Every call is INT 21h unless named otherwise; the rewrite makes the same calls in the same order (spec D1-D4 are the differences).

| Channel | Call | Use |
|---|---|---|
| DOS version | AH=30h | AL.AH below 2.00 (unsigned): `MORE: Incorrect DOS version` (AH=09h), INT 20h |
| Screen width | INT 10h AH=0Fh | AH, the text columns of the current video mode: MAXCOL |
| Console text | AH=09h | CR LF at start; `CR -- More --` at each prompt and CR LF after the key |
| Standard input | AH=45h BX=0, AH=3Eh BX=0, AH=45h BX=2 | the input is kept as a new handle; handle 0 becomes a duplicate of handle 2 (the console) |
| Input | AH=3Fh, CX=4096, the kept handle | the text; AX = 0 ends the program; carry not checked |
| Output | AH=02h, DL | every byte before a ^Z, one call each (BEL and control characters too; DOS expands TAB) |
| The key | AX=0C01h | flushes type-ahead, waits for a key from standard input (the console) and echoes it |
| Exit | INT 20h | at the end of the input or at ^Z; no exit code |

On the target (`src/main.rs`) the INT 21h calls go through `ibm_pc_dos::int21`, INT 20h through `ibm_pc_dos::exit`, and INT 10h through `more_video_columns` in `src/video.s` (AL, DX, CX clobbered as the ABI allows; BX, which INT 10h AH=0Fh sets to the page, is saved).
