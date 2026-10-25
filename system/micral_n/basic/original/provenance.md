# SCELBAL — original provenance

| File | Source URL | SHA-256 | Size | License / redistribution |
|---|---|---|---|---|
| `sc1.lst` | https://raw.githubusercontent.com/hansake/Software_for_Intel_8008/df573a420605c065ae76639d434d6bc88d22e4e5/scelbal_scelbi/sc1.lst | `b705612579f874d47ee0cf626cf58e5738961d6d0625701217c334ce5dda6c74` (`sc1.lst.sha256`) | 4627 lines | AS8 listing (T. E. Jones's assembler) of Steve Loboyko's 2001 transcription with the three published patches, corrected by Mark G. Arnold in 2012. Its header: "Copyright 1975 Scelbi Computer Consulting, Inc."; MGA permits educational, historical, non-commercial use, and the notice must travel with the program. Not committed. |
| `sc1.asm` | same commit, `scelbal_scelbi/sc1.asm` | `d2e4648711b72ff9ab1ae27bd630c72b24441fd96bc9d1d4af2cd43537da3edd` (`sc1.asm.sha256`) | 4153 lines | the source of that listing, same terms. Read for the comments; the oracle uses the listing's bytes. |

`fetch.sh` downloads both here, pinned to the repository commit, and checks
the SHA-256. `../scripts/build_original.py` takes the code bytes of
`sc1.lst` (0x0040–0x2EE5, 6862 bytes at the listing's addresses; identical
to `design/intel_8008/references/scelbal_sc1.bin` in ai_hw_engineer) and
overlays `../oracle/micral_io.asm` (a jump at 0 and the Micral N teletype in
place of the page-0 serial routines) to make `build/original.bin`, the oracle.
