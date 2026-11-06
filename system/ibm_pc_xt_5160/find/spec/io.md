# FIND: I/O contract

| Channel | DOS call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | AL below 2 (a signed comparison): print `Incorrect DOS version` (AH=09h) and return to DOS through PSP:0000h (INT 20h) |
| Switch character | INT 21h AX=3700h | DL; normally `/` |
| Command tail | PSP:0081h | read to the CR; redirection is already removed by COMMAND |
| Open | INT 21h AX=3D00h, DS:DX = the NUL-terminated name | each file name; carry: `FIND: File not found <name>` |
| Input | INT 21h AH=3Fh, CX=4096 | the opened file, or handle 0 when no name is given |
| Seek | INT 21h AX=4201h, CX:DX = minus the bytes of a partial line | before each refill that leaves a partial line; carry is a read error |
| Close | INT 21h AH=3Eh | each file at its end or after a read error (never handle 0) |
| Output | INT 21h AH=40h, handle 1 | the heading (CR LF `---------- ` + name, CR LF unless `/C`), `[n]`, each line with its line end, `: `, the count, CR LF; each piece its own call |
| Errors | INT 21h AH=40h, handle 2 | `FIND: Invalid number of parameters`, `FIND: Syntax error`, `FIND: Invalid Parameter X`, `FIND: File not found <name>`, `FIND: Read error in <name>`, each ending CR LF |
| Exit | INT 21h AH=4Ch | AL = 2 for a missing argument or a syntax error, 1 for an empty string, else 0 |

The console (DOS, not FIND) ends a function 40h write at ^Z; the host model in `tests/oracle_replay.rs` does the same.

The rewrite's target side (`src/main.rs`, `DosExe`) makes exactly these calls through `ibm_pc_dos::int21`; the
4,097-byte buffer is a static array, the string and name buffers are on the stack (tiny model: DS = SS).
