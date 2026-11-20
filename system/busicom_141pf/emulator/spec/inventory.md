# Existing Busicom 141-PF / MCS-4 emulators

Inventory for issue #23, taken 2026-11-20 from the repositories' own pages and
files. "Run" means executed or linked without copying code. Licenses are as
published; GitHub reports `NOASSERTION` where a license file is not
machine-recognised.

| Project | Language | Level | License | Runs the original ROM | Use here |
| --- | --- | --- | --- | --- | --- |
| This crate (`system/busicom_141pf/emulator`) | Rust | Instruction-cycle, board wiring | MIT (own code) | Yes; 42/42 manual examples | The project-owned emulator |
| [veniamin-ilmer/chips](https://github.com/veniamin-ilmer/chips) at `1bc3d57` | Rust (`no_std`) | Instruction-level 4004, 4001, 4002, 4003 | None stated | Via its board model | Linked, unmodified, as this crate's chip cores. Not an independent check of this crate |
| [veniamin-ilmer/busicom](https://github.com/veniamin-ilmer/busicom) at `dea0745` | Rust/WASM | Board, keyboard, drum printer, side panel | None stated | Yes | Reference for board wiring; read and run only, no code copied |
| ai_hw_engineer `system/busicom_141pf` | SystemVerilog | RTL of all five chip types plus board | Project-owned | Yes; 42/42, 846 payload lines | Primary oracle: the committed `rom_port_trace.txt` comes from it |
| [patrickleboutillier/inside4004](https://github.com/patrickleboutillier/inside4004) at `43c1a07` | Python | RTL-level, own keyboard and printer sensors | CC BY-NC-SA 4.0 (LICENSE file) | Yes; ships the same ROM (SHA-256 matches) | Independent comparator, run only, outside the repo. 42/42 identical tapes, see `report/verification.md` |
| [carlini/intel-4004-in-4004-bytes-of-c](https://github.com/carlini/intel-4004-in-4004-bytes-of-c) | C | Gate-level, microcoded | GPL-3.0 (README) | Yes | Candidate second comparator; author reports 3 to 10 minutes per calculation, so not suited to 42 examples. Not run |
| [munetomo-maruyama/MCS4_SYSTEM](https://github.com/munetomo-maruyama/MCS4_SYSTEM) | Verilog (FPGA) | RTL | CC BY-NC-SA 4.0 text in LICENSE | Reproduces the 141-PF on an FPGA | Candidate for the FPGA hand-off track (#65); not run |
| [Panda381/AT4004](https://github.com/Panda381/AT4004) | AVR assembly | 4004/4040 simulated on an ATmega8, with a 141-PF emulation | Not verified | Includes a 141-PF emulation | Hardware kit; not evaluated |

## Findings

- The only emulators that share code are this crate, `chips` and `busicom`.
  They cannot cross-check each other.
- `inside4004` and the ai_hw_engineer RTL were written separately from the
  `chips` cores and from this crate, so agreement with them is independent
  evidence.
- Software development does not need this emulator: `busicom` (WASM) and the
  RTL already run the original ROM, as #23 allows.
- `inside4004` does not model the diamond (`ST`) and `00` keys (its keyboard
  table marks them unused). The comparison script adds them at run time.
  It also never clears the red print state; paper colour is therefore not
  compared against it.
- Not checked: other general emulator frameworks (MAME, SIMH and similar) for
  an MCS-4 or Busicom driver.
