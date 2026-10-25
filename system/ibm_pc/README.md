# IBM Personal Computer 5150

CPU: Intel 8088 (8086 instruction set). Backend: `llvm-i8086` (real-mode 8086 only).

Status: **the emulator boots the unchanged IBM BIOS and PC DOS 1.00 and matches the RTL**; the backend and titles are not started. The emulator's 8088 core passes all 566,721 physical-chip vectors the RTL passes; the whole machine (8088, 8259, 8253, 8255 and keyboard, 8237, 8272, CGA text) runs a 14-step DOS and BASIC session whose screens and written disk image are identical to the RTL's ([report](emulator/report/verification.md)). No CGA graphics, speaker, compiled image, backend code or title yet.

## History

IBM's [manufacturer history](https://www.ibm.com/history/personal-computer) dates the original PC to August 1981, and its published architecture and BIOS listing helped establish the PC software and expansion-card ecosystem. The 5150 used the Intel 8088, the 8-bit-bus version of the 8086; Intel's introduction dates for the two chips (1978 and 1979 as commonly cited) are to be sourced before they are quoted in the backend README. No month-level claim is made here.

## Machine summary

- CPU and clock: Intel 8088, 4.772727 MHz (the clock the hardware project's platform uses); 8-bit external bus, four-byte prefetch queue, 20-bit physical address space.
- ROM / RAM: 8 KiB BIOS at FE000h..FFFFFh and Cassette BASIC at F6000h..FDFFFh; the reference platform models 64 KiB of RAM at 00000h..0FFFFh. The board accepted more; larger configurations are outside the first platform.
- Display / media: CGA at B8000h..BBFFFh; two 160 KiB single-sided 5.25-inch drives.
- I/O: see [platform/io_map.md](platform/io_map.md)
- Memory map: see [platform/memory_map.md](platform/memory_map.md)

## Simulators

- Local Rust system emulator: [emulator/](emulator/README.md): library and CLI that run the original firmware and, later, compiled images. Has its own 8088 core and peripherals, so it shares no code with either oracle below ([emulator/spec/spec.md](emulator/spec/spec.md)).
- Primary (ai_hw_engineer): `$AI_HW_ENGINEER/system/ibm_pc` (5150, 64 KiB, CGA, two drives) and `system/ibm_pc_xt_5160`; RTL CPU `design/intel_8086` with the 8259, 8253, 8255, 8237 and 8272 peripherals. It boots the unchanged IBM BIOS and PC DOS 1.00.
- Secondary (external/): `external/martypc` ([MartyPC](https://github.com/dbalsom/martypc), cycle-accurate IBM PC/XT emulator in Rust).

## Software

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|

No title chosen yet; candidates are in [../../llvm-i8086/plans/implementation_plan.md](../../llvm-i8086/plans/implementation_plan.md).

## Sources

- IBM, [Personal Computer Technical Reference, August 1981](https://www.minuszerodegrees.net/manuals/IBM_5150_Technical_Reference_6025005_AUG81.pdf) (motherboard schematics, I/O map, BIOS listing, keyboard, CGA and diskette adapters).
- `$AI_HW_ENGINEER/system/ibm_pc/spec/spec.md` (hardware boundary, memory and I/O decode, assumptions A1-A4).
- IBM history page above for the 1981 date.
