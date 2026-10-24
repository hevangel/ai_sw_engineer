# MIC 01 monitor — behavior spec

Source: R2E, *MICRAL N User's Manual*, January 1974, chapter II "MIC 01 Micral
Monitor" (§II.1–§II.3.2.7), in
`../../../../../ai_hw_engineer/system/micral_n/spec/reference/MICRAL_N_Users_Manual_Jan74.md`
(lines 4578–4905). The manual's example figures (II-1 to II-4) are not in the
surviving scan; pages 78–83 hold text only. The original ROM was dumped by MO5
(SHA-256 published by HxC) but its bytes are not available, so this is a
**behavioral rewrite**. The manual is the oracle. Where it leaves a format
open, the choice is recorded below as an ASSUMPTION with an ID. Code that
depends on one cites the ID.

## Machine contract

- ROM 0x3800–0x3FFF (2 KiB). Cold entry 0x3B30 (after power-on: clears the
  monitor's RAM), warm entry 0x3B49 (§II.3.1).
- Teletype and stack card as in `../platform/io_map.md`.
- The operator enters the monitor by substituting `JMP 3B30` (or `JMP 3B49`)
  on the console. Storing `C0 44 49 3B` at address 0 with `M` makes INIT enter
  it too (§II.3.3).

## Commands (§II.3.2)

The monitor waits for a command letter, echoes it and prints `:`. The
letters are M, A, H, I, D, G, C, P, F, N, R. Any other character prints `?`
and a new line, and the monitor waits again; a `?` mid-command cancels the
command the same way (§II.3.2). The monitor echoes everything it accepts,
because the ASR 33 prints only what the computer sends.

| Command | Operands | Effect | Manual |
|---|---|---|---|
| `M` | address, then values | display / modify memory | §II.3.3 |
| `A` | values | display / modify A B C D E H L | §II.3.2.3 |
| `H` | values | display / modify H L | §II.3.2.3 |
| `I` (also `T`) | values | display / modify flags C Z S P, then A–L | §II.3.2.3 |
| `D` | start `,` end CR | memory dump | §II.3.2.4 |
| `G` | address [`,` hold [`,` hold]] CR | run, with up to two holds | §II.3.2.5 |
| `C` | CR | punch leader | §II.3.2.6 |
| `P` | first `,` last CR | punch a binary block | §II.3.2.6 |
| `F` | CR | punch trailer | §II.3.2.6 |
| `N` | first `,` last CR | punch a PROM programming tape | §II.3.2.7 |
| `R` | — | read a binary tape | §II.3.2 |

### Numbers

An address is 1–4 hex digits, zero-filled on the left; a fifth digit prints
`?` (§II.3.3). It ends with CR, space or comma. A value takes the last two of
the hex digits typed; one digit is zero-filled (§II.3.3).

### M, A, H, I: display and modify

`M:` address and terminator, then the monitor prints the byte as two hex
digits and `-`. Typing a value then space or comma stores it and shows the
next location. Space or comma alone keeps the value. CR, or a character that
is not a hex digit, ends the command (§II.3.3). A, H and I step through the
saved registers the same way (§II.3.2.3). I starts with the four flags, one
digit (0/1) each.

### D: dump

`D:` start `,` end CR. Each line: CR LF, the 4-digit address of its first
column, then 16 columns of two hex digits. The first column's address has
low digit 0; columns before the start print `..`. An empty line follows each
line, for annotations (§II.3.2.4). The dump stops after the end address.

### G: go, holds

`G:` address [`,` hold1 [`,` hold2]] CR. The monitor stores a 3-byte call
into itself at each hold (holds at least 3 apart, §II.3.2.5), loads the saved
registers and jumps. Reaching a hold restores both holds' original bytes,
saves the registers, prints the hold and waits for a command. A, H and I then
show the program's registers, and `G` resumes it.

### C, P, F, N, R: paper tape

Punching goes to the teletype between DC2 (punch on) and DC4 (punch off); `R`
starts the reader with DC1 and stops it with DC3. These are the ASR 33's
controls with the punch-control and reader-control options §II.2.6 requires.

## Assumption ledger

| ID | Assumption | Why | How it could be checked |
|---|---|---|---|
| MON-1 | After the letter the monitor prints `:`; the user does not type it | "Each instruction, typed on the keyboard is followed by a colon" (§II.3.2) is ambiguous; this reading needs no extra keystroke | the original ROM, or figure II-1 |
| MON-2 | M/A/H/I print no address or register name, only `XX-` | the manual describes only the two digits and the dash | figure II-1 |
| MON-3 | `I` and `T` both mean "control and registers" | the command list says I, the text says T | the original ROM |
| MON-4 | Flags display as one digit each, in the order C Z S P | §II.3.2.3 names the order, not the format | figure II-1 |
| MON-5 | Reaching a hold prints CR LF, `*`, the hold's 4-digit address, CR LF | the manual says only that the monitor regains control | figure II-4 |
| MON-6 | Binary tape block: `:` then hex pairs for length (1–16), address high, address low, the data, and a checksum making the byte sum 0; CR LF after each block; `F` punches the end block `:00000000` before its trailer | "bloc size and initial address are punched … a check sum … each byte split in two and punched by two characters" (§II.3.2.6); the exact encoding is not given | the original ROM; a surviving R2E tape |
| MON-7 | Leader and trailer are 64 NUL characters | blank tape is NUL on an ASR 33; the length is not given | — |
| MON-8 | The PROM tape uses Intel's BNPF format (`B`, 8 × `P`/`N` from bit 7, `F`), four bytes per line | the 1702A EPROMs the Micral used were programmed from BNPF tapes; §I-4 gives no format | the original ROM |
| MON-9 | `R` reads blocks until the end block; a checksum error prints `?` and stops | the manual says the checksum is "a check when tape is read" | the original ROM |
| MON-10 | The monitor uses more than the original's one stack level and RAM beyond 0x07E0–0x07FF (see `report/final_report.md` for the measured amounts) | compiled Rust calls functions; the original was hand-written | measured by the linker map |
| MON-11 | Restoring the flags on `G` restores carry and zero; sign and parity follow from the value used to set zero | the 8008 cannot load the flags directly | — |
