# ibm_pc_xt_5160 platform

Shared by every software title on this machine.

- [memory_map.md](memory_map.md) — address map and the ROM/RAM budget every title is measured against
- [io_map.md](io_map.md) — peripherals as the software sees them
- linker scripts and runtime: the 5150's, unchanged ([../../ibm_pc/platform/](../../ibm_pc/platform/README.md)); see [../toolchain/spec/profile.md](../toolchain/spec/profile.md)
- `runtime/` — reset/crt0 code and the target implementation of each title's HAL trait
