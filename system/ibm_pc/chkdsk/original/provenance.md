# chkdsk — original provenance

| File | Source | SHA-256 | Size | License / redistribution |
|---|---|---|---|---|
| `CHKDSK.COM` | extracted from the PC DOS 1.00 boot diskette image `pcdos100.img` (directory entry `CHKDSK COM`, first cluster 31, 1395 bytes) | `b37dc82ac9317f50c4eaeb0010911dd9c0289e3eb819c327312c66dd28eb4d20` | 1395 | Copyright IBM / Microsoft. **Not redistributable: gitignored, never committed.** |
| `pcdos100.img` (the disk it came from) | `ai_hw_engineer/system/ibm_pc/work/reference/pcdos100.img`, SHA-256 `29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2`, 163,840 bytes | | | same |

`CHKDSK.COM` is reproduced from the image with `pcdisk extract` (`system/ibm_pc/platform/tools/pcdisk`):

```bash
pcdisk extract "$AI_HW_ENGINEER/system/ibm_pc/work/reference/pcdos100.img" CHKDSK.COM original/CHKDSK.COM
sha256sum -c original/CHKDSK.COM.sha256
```

Only the SHA-256 (`CHKDSK.COM.sha256`) is committed. The disassembly used to write [the spec](../spec/spec.md) was produced locally with `objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100` and is not committed either.

## Not chosen: the MIT-licensed MS-DOS 1.25 binaries

`microsoft/MS-DOS` (MIT) publishes `v1.25/bin/CHKDSK.COM` (1720 bytes). It would be redistributable, but it is a later program than the one on the PC DOS 1.00 disk that the Rust emulator and the RTL boot, and its behaviour was not observed on the machine. The oracle must be the program that runs on the reference machine, so the PC DOS 1.00 binary is the oracle; the MS-DOS 1.25 one remains a possible second oracle for a later variant.
