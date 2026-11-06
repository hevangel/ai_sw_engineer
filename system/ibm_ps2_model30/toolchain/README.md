# IBM PS/2 Model 30 compiler profile

The per-computer profile of the shared `llvm-i8086` toolchain for the IBM PS/2 Model 30 (issue [ai_sw_engineer#60](https://github.com/hevangel/ai_sw_engineer/issues/60)). It reuses the 5150 profile ([../../ibm_pc/toolchain](../../ibm_pc/toolchain/README.md)) with a 720 KiB boot diskette. **Interim: images build and pass the independent checks; not yet run on an emulator.**

- [spec/profile.md](spec/profile.md): machine, image formats, original software, assumptions
- [spec/audit.md](spec/audit.md): upstream support, emulators of the Model 30, pinned versions
- [smoke/ps2_info_rs](smoke/ps2_info_rs/src/main.rs), [expected/](expected/ps2_info.txt)
- `scripts/`: `fetch_reference.py`, `build_profile.sh`, `run_tests.sh`
- [report/report.md](report/report.md)

```bash
system/ibm_ps2_model30/toolchain/scripts/run_tests.sh
```
