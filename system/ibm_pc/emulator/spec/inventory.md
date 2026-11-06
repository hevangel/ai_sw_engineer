# Existing emulator inventory

Date: 2026-11-05. Purpose: issue #28 asks for an inventory of existing IBM PC 5150 / PC-XT-class emulators, in any language, with their licenses, machine coverage and suitability for immediate software development, so that qualified components can be reused where appropriate while this project keeps its own Rust implementation. This file records what is known and how each emulator is used here. It does not change [Decision E1](spec.md).

Licences of MartyPC, 86Box, PCem and MAME were checked on this date against the licence files on their default branches (Sources). Entries marked "verify before use" were not re-checked. Check the license text of the exact revision before copying, linking or redistributing anything.

## Emulators

| Emulator | Language | License | 5150 coverage (8088 timing, CGA, 5.25" 160K floppy, PC DOS 1.x) | Role for this project |
|---|---|---|---|---|
| MartyPC | Rust | MIT | IBM 5150 and 5160 models; cycle-accurate 8088 and bus timing, validated by its author against real hardware; CGA; 5.25" floppy; boots PC DOS 1.x. Pinned here as `external/martypc` at `05c0d088e84ad6bbfac9b3f0d051e7eadefd9f44`. | Secondary independent oracle only. Not a dependency. |
| PCjs | JavaScript | Code MIT (verify before use); archived ROMs and disks keep their original copyrights | IBM 5150 machine definitions with 8088, CGA, floppy; runs PC DOS 1.x in the browser; 8088 timing level: verify before use | Source archive for the firmware and disk images. Not used as an emulator. |
| 86Box | C/C++ | GPL-2.0 | IBM PC 5150 machine; 8088 timing at cycle level (verify before use); CGA; 5.25" floppy; PC DOS 1.x boots | Available for software development; no project use. |
| PCem | C/C++ | GPL-2.0 | IBM PC 5150 machine; CGA; floppy; 8088 timing level: verify before use | Available for software development; no project use. |
| MAME (`ibm5150` driver) | C++ | GPL-2.0 as a whole; many files also BSD-3-Clause (per file) | `ibm5150` driver with 8088, CGA, floppy; 8088 timing level: verify before use | Available for software development; no project use. |
| PCE (`ibmpc`, Hampa Hug) | C | GPL-2.0 (verify before use) | `ibmpc` model for the 5150 and XT; 8088 core; CGA; floppy; timing level: verify before use | Available for software development; no project use. |
| DOSBox | C++ | GPL-2.0+ (verify before use) | Not a 5150 model. Emulates a generic DOS PC with its own BIOS; does not run the IBM BIOS; no 8088 timing | Runs DOS programs only; unsuitable as an oracle for 5150 behavior. |
| QEMU | C | GPL-2.0 mostly (verify before use) | No 5150 or XT machine; 386-class and later PC models; no 8088, no CGA-era board | Not applicable to the 5150. |
| Bochs | C++ | LGPL-2.1+ (verify before use) | No 8088 or 5150 model; x86 from 386 up | Not applicable to the 5150. |

## Use in this project

- **RTL in `ai_hw_engineer`** is the primary oracle for the whole machine and the CPU (`design/intel_8086`, `system/ibm_pc`). The recorded sessions are in `oracle/transcripts/`.
- **MartyPC** is a secondary, independent oracle for behavior the physical-chip vectors and the RTL do not cover. It is run unmodified as a library by `tools/martypc_oracle` and driven by `scripts/martypc_boot_images.sh`, which boots the boot images and DOS programs and compares the final 25-row screen with the RTL transcript. A disagreement is a finding, not a failure of either side.
- **PCjs** is the archive from which the hardware project fetches the IBM BIOS, Cassette BASIC and PC DOS 1.00 at the pinned revisions in [original/provenance.md](../original/provenance.md). Its emulator is not run.
- **86Box, PCem, MAME, PCE, DOSBox** are available to anyone who wants to develop or try 5150 software now. The Rust emulator is not required for that work and must not block it (issue #28).
- **QEMU and Bochs** are listed only because they were asked about; neither models the 5150.
- **No code from any of these is linked into, copied into or vendored by the Rust emulator.** Every core is written in this repository (Decision E1). The only code that touches an outside emulator is `tools/martypc_oracle`, a separate test tool that builds against the unmodified `external/martypc` submodule.

## Suitability for immediate software development

For 5150 software that must behave as on the real machine (BIOS calls, CGA modes, diskette timing), MartyPC, 86Box, PCem, MAME and PCE are the candidates, with MartyPC the most documented as timing-accurate. Choosing between them needs a check of each one's current release, which was not done for this file. DOSBox is enough for DOS-level work that does not depend on the IBM BIOS. The project's own emulator already boots the IBM BIOS and PC DOS 1.00 and matches the RTL on a 20-step session ([README](../README.md)), and since 2026-11-05 has the speaker, character-ROM text frames, a debugger and a browser front end ([../web](../../web/README.md)); it is not cycle-accurate (spec E-A2), so timing-sensitive software is still better checked on MartyPC.

## Sources

- Licence files read on 2026-11-05: `raw.githubusercontent.com/dbalsom/martypc/main/LICENSE` (MIT), `86Box/86Box/master/COPYING` (GPL v2), `sarah-walker-pcem/pcem/dev/COPYING` (GPL v2), `mamedev/mame/master/COPYING` (GPL-2.0 as a whole).
- Other facts come from the repository: `README.md`, `spec/spec.md` (Decision E1), `original/provenance.md`, `scripts/martypc_boot_images.sh`, `tools/martypc_oracle/src/main.rs`, and `git submodule status` for the MartyPC revision.
- Project pages to check before use: github.com/dbalsom/martypc, github.com/jeffpar/pcjs, github.com/86Box/86Box, github.com/sarah-walker-pcem/pcem, github.com/mamedev/mame, hampa.ch/pce, dosbox.com, qemu.org, bochs.sourceforge.io.
