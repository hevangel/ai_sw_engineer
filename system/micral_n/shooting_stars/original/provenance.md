# Shooting Stars — original provenance

| File | Source URL | SHA-256 | Size | License / redistribution |
|---|---|---|---|---|
| `shooter.asm` | https://raw.githubusercontent.com/jim11662418/Intel_8008_Single_Board_Computer/main/applications/shooter.asm | `70c1b07733ef7fdeb11459fde13a2334382a3072e88f5b3559bde910eb0862c4` (`shooter.asm.sha256`) | 410 lines | Jim Loos's transcription of the BYTE (May 1976) listing for the AS assembler, in an MIT-licensed repository. The 1976 listing's own copyright status is not established, so the file is not committed. |

`fetch.sh` downloads it here and checks the SHA-256. `../scripts/build_original.sh`
appends `../oracle/micral_io.asm` (Micral N teletype in place of the SBC
routines) and assembles it into `build/original.bin`, the oracle program.
