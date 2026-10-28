# IBM PC 5150 compiler profile: report

Issue [ai_sw_engineer#58](https://github.com/hevangel/ai_sw_engineer/issues/58). Date of the run: 2026-10-27. The profile itself is [../spec/profile.md](../spec/profile.md); what was audited and pinned is [../spec/audit.md](../spec/audit.md); the commands are in [../README.md](../README.md).

## Versions

[versions.txt](versions.txt) is the output of `scripts/versions.sh` on the day: rustc 1.101.0-nightly (`7df81381d`) with the i8086 patch (sha256 `02ea97b4e1af0d5c...`), LLVM 23.1.3 (`0d261d1ca552`), the host Rust 1.99.1, GNU binutils 2.38, Capstone 5.0.7, MartyPC `05c0d088`.

## Commands and results

```bash
system/ibm_pc/toolchain/scripts/run_tests.sh     # everything below, from a clean build
```

| Step | Result |
|---|---|
| Build (`build_profile.sh`) | rustc compiles `hello_rs`, `irq_rs` and `hello_com` (Rust, `no_std`) for `i8086-unknown-none`; `llc` compiles the IR program `seg_exe`; the runtime (`crt0`, `crt0_com`, `dos`, `hal`) is assembled by `llvm-mc`; `i8086-ld` links five images with the three linker scripts. The linker refuses region overflows; the script checks the image sizes |
| Reproducible | the SHA-256 of all seven artifacts equal [artifacts.sha256](artifacts.sha256), which is committed; `hello_rs.img` and `irq_rs.img` are the very floppies the RTL booted (their hashes are line 3 of `emulator/oracle/boot/*.txt`) |
| Decoders (`check_images.py`) | 2,675 + 933 + 1,777 + 1,777 + 20 instructions in the five images, decoded by GNU binutils, Capstone and llvm-objdump: **the three agree on every instruction boundary and every byte of every code section is covered**; the mnemonics (46, 38, 44, 44 and 8 distinct) are all in the 8086 list, so no 80186+ instruction, 32-bit operand or x87 opcode is present |
| Relocations | 139, 59, 81, 81 and 7 relocations (all five kinds, `R_I8086_SEG16` included) re-applied from the object files with the map file and compared with the linked bytes: all equal |
| Placement and entry | every loaded range is inside the format's window of the memory map (floppy 0500h..FFFFh, `.COM`/`.EXE` 0100h..), no overlap; entry 7C00h for the floppies and 0100h for the DOS programs; the floppies have 55AAh at offset 510 and are 163,840 bytes; the `.COM` equals the ELF memory from 0100h; both `.EXE` files have an MZ header whose size fields, CS:IP, SS, SP and relocation table are right and whose body equals the ELF memory with every relocated word reduced by 10h |
| Negative controls (`mutants.py`) | a relocated word changed, an 80186 `pusha` placed in the code, the boot signature wiped, a loaded range moved below 0500h, a segment word of the `.EXE` changed: the checker reports each |
| Execution, RTL | the images are byte for byte the ones recorded on the hardware project's RTL with the original BIOS and PC DOS 1.00: screens in `emulator/oracle/boot/{hello_rs,irq_rs}.txt` and `oracle/dos/{hello_com,hello_exe,seg_exe}.txt` (the disks made by `pcdisk` from the profile's `.COM`/`.EXE` have the SHA-256 the RTL ran) |
| Execution, MartyPC | `run_independent.sh`: MartyPC, an emulator whose timing the author validated on real 8088 hardware, boots the same five images and shows exactly the RTL's 25 rows each (hello_rs prints `Rust on the 8088`, the computations, `done`; irq_rs counts timer ticks and keyboard scan codes; the DOS programs print `Rust under DOS`, `drive A of 2`, `12! = 479001600`, and `Far segment!` / `*` / `Near again` from the two-segment `.EXE`) |
| Execution, Rust emulator | `boot_oracle`, `i8086_dos` and `i8086_rust` (bare core, original BIOS, DOS): 12 tests pass |

## Artifacts

| File | Bytes | SHA-256 |
|---|---:|---|
| hello_rs.img | 163,840 | `c9291bf3eb7ed2099f93d41ec3311c8e895c9fbf142504c9e7bdf3b4d0c90469` |
| irq_rs.img | 163,840 | `ab2bf2165257377e67899c2b39411d7b982ef3ffe7c1ff2143f0bf8803b4ceb9` |
| hello.com | 4,474 | `931787bc9cd067a9620ba9b1d71e618f70eb316cbb192f781add038138201f7f` |
| hellox.exe | 4,986 | `78788f9ef3e06b761a5e454d5a7f533e7ad12316f06a355c9d7aae02607e2f19` |
| segx.exe | 1,617 | `db9f161eabca6401966d715cc7b88dd0fc0f9061f7701acfeee0bcf5ae45e388` |

## Limits and unsupported profiles

- Not run on a physical 5150 (none reachable from the development host); MartyPC and the RTL stand in. `llvm-i8086/rust/REAL_HARDWARE.md` has the disks and the expected screens.
- Supported: the 64 KiB CGA 5150 with two 160 KiB single-sided drives, real mode, tiny model by default, far pointers and several segments. Not supported: more than 64 KiB of RAM in the tiny model (segments are for that), other disk formats, the 8087, the cassette interface, the XT 5160 and PS/2 profiles (other issues).
- The compiler is a fork of rustc kept as a patch; upstream rustc has no 16-bit x86 target. `f128` is unsupported; floating point is software and slow.
- The independent decoders agree on instruction boundaries and 8086 membership; they do not compare operands, which `llvm-objdump`, GNU and Capstone spell differently.
- Execution evidence is for five small programs and for CHKDSK (`../../chkdsk`), not for arbitrary Rust.

## Pull request

[#113](https://github.com/hevangel/ai_sw_engineer/pull/113), branch `system/ibm_pc`.
