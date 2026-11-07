# MS-DOS 2.0 MORE source (reference, not the oracle)

`MORE.ASM` from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS) repository, `v2.0/source/MORE.ASM`
(<https://raw.githubusercontent.com/microsoft/MS-DOS/main/v2.0/source/MORE.ASM>, branch `main`, fetched 2026-11-06),
SHA-256 `b3b56fad5c9938a08de03209c21085e814b74f4299d152cd5fe2ec490c50305f`, MIT License ([LICENSE](LICENSE),
SHA-256 `b5179f78...`). Kept byte for byte (CR LF line ends). No author or revision is named in it.

It is the source of the IBM PC DOS 2.00 `MORE.COM` (384 bytes, directory date 1983-03-08) assembled with
`IBMVER EQU TRUE`, `KANJI EQU FALSE`, `MSVER EQU FALSE`, with one difference in the code:

- The binary has `dec si; inc cx` (1DCh-1DDh) after the prompt's CR LF and the reset of the counters, before
  `CHARLOOP`: the byte that filled the screen is taken again. The source goes on with the next byte. The effect
  is on screen in every case with a prompt (an empty row after `-- More --`; the 80th character of a wide line shown
  again): [../../spec/spec.md](../../spec/spec.md), S1.
- The messages and the buffer are `EXTRN` (`MOREMES.ASM`, not fetched); the binary has them right after the
  code: `CR -- More --$` at 1F0h, `MORE: Incorrect DOS version CR LF $` at 1FCh, `CR LF $` at 217h, the buffer at
  21Ah.

Everything else is the source instruction for instruction under `IBMVER`: MAXROW set to 25, MAXCOL from INT 10h
AH=0Fh, the dup of handle 0, the close of handle 0 and the dup of handle 2, the 4,096-byte reads, the column and
row rules (BEL the only byte not counted), function 0Ch with AL = 01h for the key.

Microsoft's own build in the same repository, `v2.0/bin/MORE.COM` (4,364 bytes, SHA-256 `56830889...`), is the
`MSVER` variant: no INT 10h call (MAXROW stays 24, MAXCOL 80), and no byte below 20h counted as a column; it
also has the `dec si; inc cx`. The two files differ from byte 13 on (address 010Ch: the address of the version message, which moves with the code).

Used to name the parts in `../../spec/spec.md`; the behaviour itself comes from the IBM binary and the oracle.
