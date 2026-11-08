# DEBUG: I/O contract

The core ([`src/lib.rs`](../src/lib.rs)) runs over the [`Dos`] trait. The target
([`src/main.rs`](../src/main.rs), [`src/debug.s`](../src/debug.s)) forwards to PC DOS and the BIOS;
the host test ([`tests/oracle_replay.rs`](../tests/oracle_replay.rs)) models the same calls.

| Channel | Call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | `AH.AL` below 2.00 is `Incorrect DOS version` + INT 20h |
| User segment | INT 21h AH=26h | create the program segment at `DG + 2F1h`; its word 6 (FFF0h) gives the stack top |
| Current PDB | AH=51h, 50h | set to the user segment while T/G run (so INT 20h ends the program, not DEBUG), restored after |
| Terminate | AX=2522h | INT 22h to DEBUG's handler, copied into the user PSP by function 26h, so the program's INT 20h returns to DEBUG |
| Screen width | INT 10h AH=0Fh | 40 columns switch to the narrow register/dump layout |
| Console in | AH=0Ah | the command line; the typed line and a CR are echoed, then DEBUG prints CR LF (INBUF, 2B3h) |
| Console out | AH=02h | every character DEBUG prints (bit 7 cleared); messages use AH=09h |
| Files | AH=3Dh, 3Ch, 3Eh | open (L, EXE), create (W), close |
| | AH=3Fh, 40h, 42h | read (L, EXE image and relocations), write (W), seek |
| Parse | AX=2901h | the first command-line parameter into the FCB (the file loaded on the command line) |
| Trap/breakpoint | INT 1, INT 3 | single-step (the trap flag) and breakpoints (`CC`) for T and G |
| Exit | INT 21h AH=4Ch | the Q command |

## Memory map on the oracle machine (256 KiB, PSP at 0600h)

| Paragraphs | Original | Rewrite |
|---|---|---|
| 0600h-08F0h | DEBUG's code and data (the `.COM`, 11,904 bytes, ending just below the user segment) | the PSP at 0600h (DG) with the command tail at 81h, the line buffer at E0h, the EXEC header scratch and name at 200h/300h; the rest holds what the boot left |
| 08F1h | the user program segment (function 26h: `DG + 2F1h`); for a debugged program with no file, the code assembled/dumped/traced at 08F1h:0100h | the same segment, built by the rewrite's own function 26h |
| 0905h | where `debug SORT.EXE` loads the program (PSP = `DG + 305h`; CS 0915h, SS 0939h) | the same: the rewrite reads the `.EXE` and places the PSP and image there (spec D-EXEC) |
| top of memory | COMMAND's transient part | the rewrite's image, `.bss` and stack, placed by COMMAND's EXEC for a load-high `.EXE` (spec D1); larger than the original, so it cannot load at 0600h |

The target build (`scripts/build.sh`) is linked at 100h with `link_com.ld`, the shared
`runtime/dos.s`, `src/start.s` in place of `crt0_com.s`, and `src/debug.s`, then wrapped by
`scripts/mkexe.py` into the load-high `.EXE` shipped as DEBUG.COM: a 32-byte MZ header
(MINALLOC = MAXALLOC = 0, CS = SS = FFF0h, IP = 100h) and the image with `.bss` and a 2 KiB stack
as zeros. `src/debug.s` has `debug_general` (a general INT 21h from a register block), `debug_out`,
`debug_line`, `debug_columns`, `debug_create_psb`, `debug_set_terminate`, `debug_peek`/`debug_poke`
on an arbitrary segment, `debug_exit` (INT 20h with CS = the PSP's segment), and `debug_run` (the
DEXIT/REENTER trace/go engine).
