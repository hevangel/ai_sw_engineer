# IBM PC XT 5160 compiler profile: report

Issue [ai_sw_engineer#59](https://github.com/hevangel/ai_sw_engineer/issues/59). Run of 2026-11-06 on a Linux host (4 cores; no container image). The profile is [../spec/profile.md](../spec/profile.md), the audit and versions [../spec/audit.md](../spec/audit.md).

## Commands

```bash
python3 -I system/ibm_pc_xt_5160/toolchain/scripts/fetch_reference.py DIR
```

```bash
XT_ROMS=DIR system/ibm_pc_xt_5160/toolchain/scripts/run_tests.sh
```

## Results

| Step | Result |
|---|---|
| Profile consistency (`tests/test_profile.py`) | 16 of 16: the profile states the backend's D1 data layout exactly, the Rust route uses it, no second XT target or XT-only linker script exists, the build uses `i8086-ld` and the 5150 platform files |
| Build (`build_profile.sh`) | six images from source: `xt_info.img`, `hello_rs.img`, `irq_rs.img` (bootable 160 KiB floppies), `hello.com`, `hellox.exe`, `segx.exe`; SHA-256 in [artifacts.sha256](artifacts.sha256). **The five images the 5150 profile also builds are byte-identical to its committed artifacts** (`system/ibm_pc/toolchain/report/artifacts.sha256`), built here by the msp430 Rust route against a freshly built LLVM: the toolchain is the one qualified for the 5150 |
| Decoders (`check_images.py`) | 1,449 + 2,675 + 933 + 1,777 + 1,777 + 20 instructions: GNU binutils, Capstone and llvm-objdump agree on every instruction boundary and cover every code byte; every mnemonic is in the 8086 list (no 80186+, 32-bit operand or x87 instruction) |
| Relocations, placement, entry, formats | 84, 59, 81, 81 and 7 relocations re-applied from the objects equal the linked bytes (plus `hello_rs`'s); every loaded range inside its window (floppy 0500h..FFFFh, DOS 0100h..); entries 7C00h and 0100h; floppies 160 KiB with 55AAh; `.COM` = ELF memory; MZ headers right |
| Negative controls (`mutants.py`, `run_tests.sh`) | a changed relocated word, an 80186 `pusha`, a wiped boot signature, a range below 0500h, a changed `.EXE` segment word, and a wrong expected RAM size are each reported |
| Execution on MartyPC's 5160 (`run_independent.sh`) | see below: 6 of 6 |

### Execution

MartyPC `05c0d088` (unmodified, its 8088 validated against real hardware by its author), IBM 5160 model, 256 KiB, CGA, two diskette drives, the original 1982-11-08 XT BIOS and Cassette BASIC C1.10. Expectations do not come from MartyPC:

| Image | Expected | Result |
|---|---|---|
| `xt_info.img` (Rust, the XT smoke program) | [../expected/xt_info.txt](../expected/xt_info.txt), written before the first run: `ram = 256 KiB`, `drives = 2`, `video = 80x25 colour` (the profile's machine as the BIOS stores it at 0040:0013 and 0040:0010), `blocks above 64 KiB = 12, bad = 0` (every 16 KiB block up to 256 KiB written and read back through far pointers), `bios sum = 0x2443` (computed from the ROM file on the host) | identical |
| `hello_rs.img` (Rust: `core::fmt`, 16/32/64-bit arithmetic, sort) | the screen the ai_hw_engineer RTL recorded for the same image on the 5150 (`emulator/oracle/boot/hello_rs.txt`; the image's SHA-256 is the one in that transcript) | identical |
| `irq_rs.img` (Rust timer and keyboard interrupt handlers; keys `abc` typed) | the RTL's screen (`oracle/boot/irq_rs.txt`) | identical |
| `hello.com`, `hellox.exe` under the original **PC DOS 2.00** | the lines the program printed under PC DOS 1.00 on the RTL: `Rust under DOS`, `drive A of 2`, `12! = 479001600` | identical |
| `segx.exe` (two segments, three segment relocations) under PC DOS 2.00 | `Far segment!`, `*Near again` | identical |

The DOS disks are the original PC DOS 2.00 disk 1 (180 KiB) with the program added by `pcdisk`; PC DOS 2.00's own `DIR` reports 31,232 bytes free on the original disk, the 61 free clusters `pcdisk` counts.

## Artifacts

| File | Bytes | SHA-256 |
|---|---:|---|
| xt_info.img | 163,840 | `4104f12f8424738f0fb920ff5f1f3548fc8e4e1973cd91f47831863d8a00ed6d` |
| hello_rs.img | 163,840 | `c9291bf3eb7ed2099f93d41ec3311c8e895c9fbf142504c9e7bdf3b4d0c90469` |
| irq_rs.img | 163,840 | `ab2bf2165257377e67899c2b39411d7b982ef3ffe7c1ff2143f0bf8803b4ceb9` |
| hello.com | 4,474 | `931787bc9cd067a9620ba9b1d71e618f70eb316cbb192f781add038138201f7f` |
| hellox.exe | 4,986 | `78788f9ef3e06b761a5e454d5a7f533e7ad12316f06a355c9d7aae02607e2f19` |
| segx.exe | 1,617 | `db9f161eabca6401966d715cc7b88dd0fc0f9061f7701acfeee0bcf5ae45e388` |

## Limits and unsupported profiles

- Not run on a physical XT, nor on the hardware project's XT RTL (not reachable from this session); MartyPC is the execution oracle, the RTL's 5150 recordings the expectation for the shared programs.
- The images were built by the msp430 Rust route, not the patched rustc; the 5150 profile showed the two give byte-identical programs, and the identical hashes above confirm it for these five.
- Boot floppies are the 160 KiB format (read by the XT's drive); 180/360 KiB boot images, booting from the fixed disk, and the 8087 in generated code are not supported.
- The decoders check instruction boundaries and 8086 membership, not operand spelling.
- Evidence is for six programs, not arbitrary Rust.
