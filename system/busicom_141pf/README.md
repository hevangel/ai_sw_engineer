# Busicom 141-PF

CPU: i4004. Backend: `llvm-i4004`.

## History

The Busicom 141-PF printing calculator (Nippon Calculating Machine Corp., sold as Busicom, and as Unicom 141 in the US) was introduced in 1971 and was the first commercial product built around a microprocessor, the Intel 4004 of the MCS-4 chip set ([IPSJ Computer Museum](https://museum.ipsj.or.jp/en/heritage/Busicom_141-PF.html)). The sibling ai_hw_engineer reconstruction dates it to October 1971; the month is not asserted here. The firmware was written by Intel (Federico Faggin, Masatoshi Shima and colleagues) to Busicom's specification; its reverse-engineering and annotated disassembly is the work of the Busicom 141-PF Replication Project (B. and B. Silverman, E. Dvorak, L. Kintli; [4004.com](https://www.4004.com)).

## Machine summary

- CPU: Intel 4004, ~740 kHz, 8 clocks per machine cycle
- ROM: 5 × 4001 = 1280 bytes (the original firmware fills it); RAM: 2 × 4002 (variant -1)
- I/O: 3 × 4003 shift registers (keyboard scan, 20-bit printer hammer chain), drum printer, three lamps, two selector switches — see [platform/io_map.md](platform/io_map.md)
- Memory map: [platform/memory_map.md](platform/memory_map.md)

## Simulators

- Primary: `$AI_HW_ENGINEER/system/busicom_141pf` — 4004/4001/4002/4003 RTL running the original ROM, web front panel, headless manual replay (`scripts/replay_manual.py`).
- Secondary: `external/busicom` + `external/chips` — V. Ilmer's Rust MCS-4 emulator and board model; ai_hw_engineer already cross-checks its RTL against it (`scripts/check_reference.py`).

## Software

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|
| Calculator firmware | 1971 | Intel for Busicom | M1: host core, addition/subtraction | [firmware/](firmware/README.md) |

## Sources

- *Unicom 141 Series Calculator Operating Instructions* ([Internet Archive](https://archive.org/details/Unicom141PManual)); normalized copy and 42 machine-readable examples in `$AI_HW_ENGINEER/system/busicom_141pf/spec/reference/`.
- ai_hw_engineer [system spec](https://github.com/hevangel/ai_hw_engineer/blob/main/system/busicom_141pf/spec/spec.md) (board wiring, keyboard matrix, printer drum).
- Busicom 141-PF Replication Project annotated firmware source (CC BY-NC-SA 2.5), `Busicom-141PF-Calculator_asm_rel-1-0-1.txt` in the same reference folder — cited, not copied.
