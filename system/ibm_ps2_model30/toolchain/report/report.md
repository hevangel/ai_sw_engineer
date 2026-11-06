# IBM PS/2 Model 30 compiler profile: report

Issue [ai_sw_engineer#60](https://github.com/hevangel/ai_sw_engineer/issues/60). Run of 2026-11-06 on a Linux host. Profile: [../spec/profile.md](../spec/profile.md); audit and versions: [../spec/audit.md](../spec/audit.md).

## Commands

```bash
python3 -I system/ibm_ps2_model30/toolchain/scripts/fetch_reference.py DIR
```

```bash
PS2_ROMS=DIR BOX86=<86Box> system/ibm_ps2_model30/toolchain/scripts/run_tests.sh
```

`BOX86` is 86Box built from commit `a1e92633` with `-D AUTOMATION=ON` ([../spec/profile.md](../spec/profile.md), "Execution oracle").

## Results

| Step | Result |
|---|---|
| Original software (`fetch_reference.py`) | BIOS chips, the 64 KiB BIOS assembled from them, PC DOS 3.30 disk 1 and the 720 KiB boot disk made from it: all five SHA-256 as pinned, the same on a second run |
| Build (`build_profile.sh`) | six images from source: `ps2_info.img`, `hello_rs.img`, `irq_rs.img` (bootable 720 KiB diskettes with the PS/2 `crt0.s`), `hello.com`, `hellox.exe`, `segx.exe` (byte-identical to the 5150 profile's: the DOS formats do not depend on the machine); SHA-256 in [artifacts.sha256](artifacts.sha256) |
| Decoders (`check_images.py`) | 1,550 + 2,685 + 943 + 1,777 + 1,777 + 20 instructions: GNU binutils, Capstone and llvm-objdump agree on every boundary and cover every code byte; all mnemonics 8086. The first build failed here, rightly: the first `crt0.s` kept the boot drive as a data byte at the end of the boot code, which the decoders cannot agree on; it now keeps it in BP |
| Relocations, placement, entry, formats | 97, 139, 59, 81, 81, 7 relocations re-applied equal the linked bytes; ranges inside 0500h..FFFFh (diskettes) and 0100h.. (DOS); entries 7C00h and 0100h; diskettes 720 KiB with 55AAh, boot sector and stage 2 equal the ELF memory |
| Negative controls (`mutants.py`) | a changed relocated word, an 80186 `pusha`, a wiped boot signature, a range below 0500h, a changed `.EXE` segment word: each reported |
| Execution on 86Box's PS/2 Model 30 (`run_independent.sh`) | **6 of 6**, below; a wrong expected model byte is reported (negative control) |

### Execution

86Box `a1e92633` (unmodified; GPL-2.0), `ibmps2_m30` with BIOS revision 0, 8086 at 8 MHz, 640 KiB, MCGA, one 3.5-inch 720 KiB drive. POST shows `161-Set Date and Time` and `1790-Fixed Disk Error` (true of this machine: no clock set, no fixed disk) and waits for F1, which the driver presses. Expectations do not come from 86Box:

| Image | Expected | Result |
|---|---|---|
| `ps2_info.img` | [../expected/ps2_info.txt](../expected/ps2_info.txt): 1 drive, display MCGA colour 0Ch (INT 10h function 1Ah), model byte FAh and date 09/02/86 (read from the ROM file), 36 blocks above 64 KiB none bad, ROM sum 3A40h (computed from the file), and RAM | identical, after one correction (below) |
| `hello_rs.img`, `irq_rs.img` (720 KiB boot diskettes) | the screens the RTL recorded for these programs on the 5150 | identical; `irq_rs` takes the timer and keyboard interrupts in Rust and prints the six codes typed (P-A2) |
| `hello.com`, `hellox.exe` under the original **PC DOS 3.30** (booted from the 720 KiB disk, P-A3) | the RTL's lines, with `drive A of 5` (below) | identical |
| `segx.exe` (two segments) under PC DOS 3.30 | `Far segment!`, `*Near again` | identical |

Two expectations were wrong at first and were corrected from documentation, not from the emulator:

- **RAM:** written as 640 KiB; 86Box showed 639. The PS/2 BIOS takes the top 1 KiB of conventional memory for its extended BIOS data area and reports the rest (IBM, *PS/2 and PC BIOS Interface Technical Reference*). Every 16 KiB block up to that top passed the write/read test.
- **Drive count:** the run script first expected `drive A of 1` (the Model 30's one drive). `hello_com` prints what DOS function 0Eh returns, which from DOS 3.0 on is the number of logical drive letters, at least 5 (Microsoft, *MS-DOS Programmer's Reference*, INT 21h function 0Eh); PC DOS 1.00 and 2.00 return the real count, which is why the 5150 and the XT print 2. 86Box under DOS 3.30 printed 5.

## Artifacts

| File | Bytes | SHA-256 |
|---|---:|---|
| ps2_info.img | 737,280 | `c49c7ce94136bd563e5dad87ace5de50fe5b33fdf16bf217e46424a6e7dc63fb` |
| hello_rs.img | 737,280 | `9429b648cf1f22e2939dd76bfb236e3afc1bbac610ff1fe6758abc0c86f96430` |
| irq_rs.img | 737,280 | `aa51de840e5909160b5810d8c95f47e6b85935414ddc9204b5b9d62336a3b34f` |
| hello.com | 4,474 | `931787bc9cd067a9620ba9b1d71e618f70eb316cbb192f781add038138201f7f` |
| hellox.exe | 4,986 | `78788f9ef3e06b761a5e454d5a7f533e7ad12316f06a355c9d7aae02607e2f19` |
| segx.exe | 1,617 | `db9f161eabca6401966d715cc7b88dd0fc0f9061f7701acfeee0bcf5ae45e388` |

## Limits

- Not run on hardware or on the hardware project's RTL; 86Box is the execution oracle. It was built from its development branch (the automation socket is not in a release yet), pinned by commit.
- POST errors 161 and 1790 are passed with F1; the machine has no clock setting or fixed disk.
- The DOS disk is made from PC DOS 3.30's files (P-A3), not an original 720 KiB distribution disk.
- Evidence is for six programs, not arbitrary Rust.
