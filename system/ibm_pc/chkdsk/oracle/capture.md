# How the oracle was captured

Every transcript in `transcripts/` is the **original** `CHKDSK.COM` of PC DOS 1.00 (SHA-256 in [provenance](../original/provenance.md)) running under the unchanged PC DOS 1.00 on the ai_hw_engineer RTL system build (`Vibm_pc`: unchanged IBM BIOS ROM, 8088 RTL, 8259/8253/8255/765/6845 RTL models, 64 KiB RAM), driven by [`scripts/rtl_oracle.sh`](../scripts/rtl_oracle.sh):

1. drive A = `pcdos100.img` (the reference PC DOS 1.00 disk), drive B = the case's disk image;
2. power on, answer the date prompt with `10-14-81` by scan codes through the keyboard port, wait for the `A>` prompt;
3. type the command, wait for the next `A>`;
4. record the 25 text rows of the CGA text screen, the final CPU registers, the SHA-256 of both disks before, and of drive B after (the image is saved to `build/oracle/<name>.after.img`).

Transcript format (differs from the template's hex format because the output is a screen, not a byte stream): `#` comments, `> <command typed>`, `|<screen row>`.

One run takes about three minutes of host time (a full RAM test and DOS boot).

| File | Disk B | Command |
|---|---|---|
| `001_workdisk.txt` | `workdisk.img` of the hardware project (a formatted blank data disk) | `chkdsk b:` |
| `002_blank.txt` | `pcdisk blank` | `chkdsk b:` |
| `003_dosdisk.txt` | `pcdos100.img` | `chkdsk b:` |
| `004`–`014` | the damaged disks of `scripts/make_disks.py` | see the table in [the spec](../spec/spec.md) |
