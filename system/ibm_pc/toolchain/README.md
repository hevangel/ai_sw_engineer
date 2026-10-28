# IBM PC 5150: Rust compiler configuration, runtime and linker profile

The per-computer profile of the shared `llvm-i8086` toolchain: [spec/profile.md](spec/profile.md) says what the machine is and how Rust maps to it, [spec/audit.md](spec/audit.md) what exists upstream and the pinned versions, [report/report.md](report/report.md) what was built and verified. Issue: [ai_sw_engineer#58](https://github.com/hevangel/ai_sw_engineer/issues/58). The backend is in [../../../llvm-i8086](../../../llvm-i8086/README.md); the runtime, linker scripts and HAL are in [../platform](../platform/README.md).

```bash
system/ibm_pc/toolchain/scripts/build_profile.sh    # in the docker image: Rust -> LLVM -> objects -> five images
system/ibm_pc/toolchain/scripts/check_images.py build /workspace   # three decoders, relocations, placement
system/ibm_pc/toolchain/scripts/run_independent.sh  # the images on MartyPC against the RTL's screens
system/ibm_pc/toolchain/scripts/run_tests.sh        # all of it, from the host
```

Build a program of your own: a crate with `ibm_pc_hal` (boot floppy) or `ibm_pc_dos` (DOS), compiled by `llvm-i8086/scripts/build_rust.sh <crate> <name> <out>` and linked with one of the scripts in `../platform` (see `build_profile.sh` for the exact `i8086-ld` lines).
