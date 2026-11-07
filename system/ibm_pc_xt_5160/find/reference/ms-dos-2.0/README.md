# MS-DOS 2.0 FIND source (reference, not the oracle)

`FIND.ASM` from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS) repository, `v2.0/source/FIND.ASM`
(<https://raw.githubusercontent.com/microsoft/MS-DOS/main/v2.0/source/FIND.ASM>, branch `main`, fetched 2026-11-06),
SHA-256 `487307c6982c72f544b6f8a89dd99ff16ee949a8f00fa298f5c7048a94edaba4`, MIT License ([LICENSE](LICENSE),
SHA-256 `b5179f78...`). Kept byte for byte (CR LF line ends). Written by M. A. Ulloa (Microsoft), revisions
V1.1 of 8/23/82 to Rev. 8 of 04/14/83.

It is **Rev. 8**; the IBM PC DOS 2.00 `FIND.EXE` this title reimplements (directory date 1983-03-08) is **Rev. 6**
(11/10/82):

- The binary's first bytes are `MAUlloa/Microsoft/V12` followed by the revision byte `6`; the source has
  `***MAUlloa/Microsoft/V12***` and `8`.
- Rev. 7 (03/30/83) added a 512-byte patch area before the buffers; the binary has none: the string length is at
  332h, right after the code, then the string buffer (334h), the name length (3B4h), the name (3B6h) and the
  4,097-byte file buffer (3F7h), with the messages at 1400h.
- Rev. 8 (04/14/83) added Kanji matching under `KANJI equ FALSE`; with it off, the binary's matcher (1E1h-1FCh) is
  the source's `ELSE` branch instruction for instruction.
- The file-name errors differ: the binary prints the message, the name, then CR LF (its `prt_file_name`, 301h,
  writes the CR LF itself) and has no `errmsg3_post`/`errmsg4_post` text; the source prints `_pre`, the name and a
  `_post` message, with no CR LF in `prt_file_name`.
- The messages are in a separate module the repository does not have (`EXTRN` at the end of the source); the
  binary's text is in [../../spec/spec.md](../../spec/spec.md) (`Incorrect DOS version` has no `FIND:` and no
  CR LF).

Microsoft's own build in the same repository, `v2.0/bin/FIND.EXE` (5,796 bytes, SHA-256 `d57bab73...`), has the
**same load module** as IBM's, byte for byte (also Rev. 6); the two files differ only in the MZ header (checksum,
relocation-table offset 1Ch vs 20h) and IBM's 92 zero bytes after the image (5,888 bytes on disk; the header
gives 5,796 bytes of image).

Everything else (the option scan, the quote rules, the 4,096-byte buffer with the seek back over a partial
line, the line and match counters, the output format) is the same in the source and the binary. Used to name
the parts in `../../spec/spec.md`; the behaviour itself comes from the IBM binary and the oracle.
