# IBM PC XT 5160 compiler profile

The per-computer profile of the shared `llvm-i8086` toolchain for the IBM PC XT 5160 (issue [ai_sw_engineer#59](https://github.com/hevangel/ai_sw_engineer/issues/59)). The XT reuses the 5150 profile ([../../ibm_pc/toolchain](../../ibm_pc/toolchain/README.md)): the same backend, target, runtime and linker scripts.

- [spec/profile.md](spec/profile.md): the machine, how Rust maps to it, image formats, original software
- [spec/audit.md](spec/audit.md): upstream support and pinned versions
- [smoke/xt_info_rs](smoke/xt_info_rs/src/main.rs): the XT smoke program (BIOS data area, RAM above 64 KiB, ROM sum)
- [expected/](expected/xt_info.txt): expected screens, written before the runs
- `scripts/`: `fetch_reference.py` (original ROMs and PC DOS 2.00), `build_profile.sh`, `run_independent.sh` (MartyPC 5160), `compare_screen.py`, `run_tests.sh` (everything)
- [report/report.md](report/report.md): versions, commands, results, artifacts and hashes, limits

```bash
python3 -I system/ibm_pc_xt_5160/toolchain/scripts/fetch_reference.py DIR
```

```bash
XT_ROMS=DIR system/ibm_pc_xt_5160/toolchain/scripts/run_tests.sh
```
