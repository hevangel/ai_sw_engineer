# IBM PS/2 Model 30 compiler profile: report

Issue [ai_sw_engineer#60](https://github.com/hevangel/ai_sw_engineer/issues/60). Run of 2026-11-06 on a Linux host. **Interim: the execution gate is not done** (no image has run on an emulator of the Model 30 yet). Profile: [../spec/profile.md](../spec/profile.md); audit and versions: [../spec/audit.md](../spec/audit.md).

## Commands

```bash
python3 -I system/ibm_ps2_model30/toolchain/scripts/fetch_reference.py DIR
```

```bash
system/ibm_ps2_model30/toolchain/scripts/run_tests.sh
```

## Results

| Step | Result |
|---|---|
| Original software (`fetch_reference.py`) | BIOS chips, the 64 KiB BIOS assembled from them, PC DOS 3.30 disk 1 and the 720 KiB boot disk made from it: all five SHA-256 as pinned, the same on a second run |
| Build (`build_profile.sh`) | six images from source: `ps2_info.img`, `hello_rs.img`, `irq_rs.img` (bootable 720 KiB diskettes with the PS/2 `crt0.s`), `hello.com`, `hellox.exe`, `segx.exe` (byte-identical to the 5150 profile's: the DOS formats do not depend on the machine); SHA-256 in [artifacts.sha256](artifacts.sha256) |
| Decoders (`check_images.py`) | 1,550 + 2,685 + 943 + 1,777 + 1,777 + 20 instructions: GNU binutils, Capstone and llvm-objdump agree on every boundary and cover every code byte; all mnemonics 8086. The first build failed here, rightly: the first `crt0.s` kept the boot drive as a data byte at the end of the boot code, which the decoders cannot agree on; it now keeps it in BP |
| Relocations, placement, entry, formats | 97, 139, 59, 81, 81, 7 relocations re-applied equal the linked bytes; ranges inside 0500h..FFFFh (diskettes) and 0100h.. (DOS); entries 7C00h and 0100h; diskettes 720 KiB with 55AAh, boot sector and stage 2 equal the ELF memory |
| Negative controls (`mutants.py`) | a changed relocated word, an 80186 `pusha`, a wiped boot signature, a range below 0500h, a changed `.EXE` segment word: each reported |
| **Execution** | **not run.** Expected screens are written ([../expected/ps2_info.txt](../expected/ps2_info.txt): 640 KiB, one drive, MCGA colour 0Ch, model byte FAh, BIOS date 09/02/86, 36 blocks above 64 KiB, ROM sum 3A40h); `hello_rs`/`irq_rs` would be compared with the RTL's recorded screens and the DOS programs with their recorded lines, as in the XT profile |

## Why execution is not done

No emulator of the 8086 Model 30 could be run in this session: MartyPC and MAME 0.264 do not model it ([../spec/audit.md](../spec/audit.md)). 86Box does and has an automation socket that can drive it headless, but building it means compiling 86Box's current development source, which this session was not permitted to do without the owner's approval. A side attempt to exercise the 720 KiB loader on MartyPC's XT with the 1986 XT BIOS stopped at that BIOS's POST diskette error 601 with any drive type, so it says nothing about the loader.

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

Everything in "Why execution is not done"; plus the assumptions P-A1..P-A3 of the profile, each to be checked by the execution run; no hardware or RTL run; evidence is for six programs.
