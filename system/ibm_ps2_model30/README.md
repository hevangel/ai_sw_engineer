# IBM Personal System/2 Model 30

CPU: Intel 8086 at 8 MHz. Backend: `llvm-i8086` (shared with the IBM PC 5150 and XT 5160).

## History

IBM introduced the Personal System/2 line in 1987 (IBM, *Model 30 Technical Reference*, 1987);
the Model 30 (8530-002/021) was its entry model, keeping the 8086 and the PC's ISA-style bus while bringing
the new 3.5-inch 720 KiB diskette and the MCGA display adapter on the system board. No month-level
claim is made here.

## Machine summary

- CPU and clock: Intel 8086, 8 MHz, real mode
- ROM / RAM: 64 KiB BIOS at F0000h (with Cassette BASIC); 640 KiB RAM
- Display / media: MCGA on the system board; one 3.5-inch 720 KiB diskette drive
- Memory map: [platform/memory_map.md](platform/memory_map.md); I/O: [platform/io_map.md](platform/io_map.md)

## Toolchain

[toolchain/](toolchain/README.md): the compiler profile of this computer (issue
[ai_sw_engineer#60](https://github.com/hevangel/ai_sw_engineer/issues/60)). Status 2026-11-06: six
images build reproducibly and pass the independent decoder, relocation and placement checks; the
execution gate (running them on an independent emulator of the Model 30) is not done yet, see the
[report](toolchain/report/report.md).

## Simulators

- Rust system emulator: [emulator/](emulator/README.md) is the scaffold's placeholder; the
  emulator is issue [ai_sw_engineer#30](https://github.com/hevangel/ai_sw_engineer/issues/30).
- Independent emulator for the profile: 86Box (`[8086] IBM PS/2 model 30`, `ibmps2_m30`), with its
  automation socket; not yet built here (see the report).
- Primary (ai_hw_engineer): the SV machine of hevangel/ai_engineer#86, not used here.

## Software

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|
| — | — | — | gated by the toolchain profile | — |

## Sources

- IBM, *Personal System/2 Model 30 Technical Reference* (1987): memory map, I/O, BIOS interface.
- BIOS revision 0 (09/02/86), chips 68X1687 (even) and 68X1627 (odd), from the 86Box ROM set at
  commit `c4cd8da9`; see [toolchain/spec/profile.md](toolchain/spec/profile.md).
