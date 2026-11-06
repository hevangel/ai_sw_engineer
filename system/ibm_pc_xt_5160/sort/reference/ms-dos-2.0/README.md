# MS-DOS 2.0 SORT source (reference, not the oracle)

`SORT.ASM` from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS) repository, `v2.0/source/SORT.ASM`
(branch `main`, fetched 2026-11-06), SHA-256 `4d99cb8ab2772d7f228d5cab27f3650e40728c5738865ae2db6aad67b3cf3b49`,
MIT License ([LICENSE](LICENSE), SHA-256 `b5179f78...`). Kept byte for byte (CR LF line ends).

It is **not** the source of the IBM PC DOS 2.00 `SORT.EXE` this title reimplements:

- Microsoft's built `v2.0/bin/SORT.EXE` (1,216 bytes, SHA-256 `f2135e16...`) differs from IBM's (1,280 bytes, `755a3c1c...`).
- The source compares keys through a collating table (`internat equ true`, `mov bx,offset dg:table`);
  IBM's binary compares raw bytes with `repz cmpsb`, case-sensitive (oracle case c01).
- Its modification history (Chris Peters; "3-18-83 MZ Fix CR-LF at end of buffer, Fix small file sorting,
  Fix CR-LF line termination bug") is later than the IBM file's directory date (3-08-83); the IBM binary
  shows the behaviours those fixes address (an extra empty line for input ending in CR LF, case c10).

Used to name the algorithm's parts in `../../spec/spec.md`; the behaviour itself comes from the IBM binary
and the oracle.
