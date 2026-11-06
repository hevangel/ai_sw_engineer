# ibm_ps2_model30 runtime

- `crt0.s`: boot sector and start for the 720 KiB diskette (9 sectors, 2 heads; loads the image
  from the boot drive in logical sector order, clears `.bss`, calls `main`), linked with `../link.ld`.
- `ps2.s`: `ps2_display_combination()` (INT 10h AX=1A00h).
- Shared, unchanged, from `system/ibm_pc/platform/runtime`: `hal.s` (BIOS teletype, port I/O, far
  memory, vectors) with the `ibm_pc_hal` crate, `crt0_com.s` and `dos.s` for DOS programs.

See [../../toolchain/spec/profile.md](../../toolchain/spec/profile.md).
