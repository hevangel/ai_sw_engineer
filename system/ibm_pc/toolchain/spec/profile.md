# IBM PC 5150 compiler profile

What "Rust for the IBM PC 5150" means in this repository: the exact machine, the exact instruction set, how Rust maps to it, and what an image must look like. It is the profile of the shared `llvm-i8086` backend ([../../../../llvm-i8086/spec/](../../../../llvm-i8086/spec/abi.md)) for this computer; the backend itself is not duplicated here. Issue: [ai_sw_engineer#58](https://github.com/hevangel/ai_sw_engineer/issues/58).

## Machine and CPU

| | |
|---|---|
| Computer | IBM Personal Computer 5150 (1981), motherboard with 64 KiB, CGA, two 160 KiB single-sided 5.25-inch drives, original BIOS (SHA-256 `a259306c...`) and Cassette BASIC; memory map in [../../platform/memory_map.md](../../platform/memory_map.md), I/O map in [../../platform/io_map.md](../../platform/io_map.md) |
| CPU | Intel 8088 at 4.772727 MHz: the 8086 instruction set, 8-bit external bus, four-byte prefetch queue, 20-bit physical addresses |
| Mode | real mode only. Segment registers are set by the program; the tiny model (CS = DS = ES = SS) is the default |
| Endianness | little |
| Coprocessor | none: the 8087 socket is empty on the reference machine, so no `esc` instruction is ever generated |

### Instruction set, and what is forbidden

The backend selects, assembles and disassembles the **8086/8088 instruction set of Intel's 1979 manual (9800722-03) and nothing else**, enforced by the assembler, not by convention:

- accepted: every 8086 integer instruction, the string instructions with `rep`/`repz`/`repnz`, `loop`/`jcxz`, `int`/`iret`, far `call`/`jmp`/`ret`, segment-override prefixes, `lock`, `wait`/`hlt`;
- rejected on purpose (`llvm-mc` reports an error; the decoder reports invalid): the 80186 additions (`pusha`, `popa`, `bound`, `enter`, `leave`, `ins`/`outs`, `push imm`, `imul r,r/m,imm`, shifts and rotates by an immediate count other than 1), the `0F` opcode map, operand- and address-size prefixes (32-bit registers), `fs`/`gs`, protected-mode instructions, and `esc` (x87);
- the generated code uses only the set above: `scripts/check_images.py` decodes every linked image with GNU binutils and with Capstone and fails on any mnemonic outside the 8086 list.

## Rust and LLVM representation

| | |
|---|---|
| rustc | a fork of the pinned nightly (rust-lang/rust 7df81381d, nightly-2026-10-15) that adds the target `i8086-unknown-none` ([../../../../llvm-i8086/rust/patches/](../../../../llvm-i8086/rust/patches/apply_i8086_target.py)); built against our LLVM; `core` is built for it with `-Z build-std=core` |
| Target | `i8086-unknown-none`: `no_std`, `panic=abort`, no atomics (one core; interrupts are the only concurrency), `c_int` is 16 bits, `relocation-model=static` |
| Pointers and data layout | `e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16` (backend decision D1): near pointers are 16 bits (an offset in the program's segment); address space 1 is the 32-bit far data pointer, offset in the low word and segment in the high word. `usize` is 16 bits |
| Integers | 8, 16, 32 and 64 bits natively through carry chains; multiply, divide and remainder of 32 and 64 bits through the runtime library; `i128` through `compiler_builtins` |
| Floating point | software only: `f32` and `f64` through `compiler_builtins` (IEEE, bit-exact against the host); `f16` through its conversions; **`f128` is unsupported** (reported as unreliable to `core`); no libm beyond what `compiler_builtins` provides |
| Calling convention | backend decision D3: arguments in AX, DX, CX then on the stack (right to left); results in AX (DX:AX for 32 bits); BX, SI, DI and BP are preserved; the direction flag is clear at entry and exit. Variadic functions pass everything on the stack (D3a). Far functions use `cc 1000` (`retf`, D8). `extern "msp430-interrupt"` is the interrupt handler ABI (D5: saves every register, DS = ES = CS, `iret`) |
| Object format | ELF32 relocatable, `EM` 0x8086; relocations `R_I8086_8`, `_16`, `_PC8`, `_PC16` and `_SEG16` (`sym@seg`) |
| Linker | `i8086-ld` ([../../../../llvm-i8086/asm/](../../../../llvm-i8086/asm/README.md)): GNU-script subset, section garbage collection, archives, partial links, several segments, `.COM`, `.EXE` with relocations and the 160 KiB floppy |

An x86 generic target (`i686-*`, `i586-*`) is not usable and was not considered a starting point: it assumes 32-bit registers, a flat 32-bit address space and instructions from the 80386 on. The only upstream 16-bit LLVM target of the right shape is MSP430, whose data layout the project used as a stand-in until the real target existed ([../../../../llvm-i8086/rust/README.md](../../../../llvm-i8086/rust/README.md)).

## Image formats and the memory map

Three formats, each with a linker script in [../../platform/](../../platform/):

| Format | Linker script | Start-up | Entry and layout | Tool |
|---|---|---|---|---|
| Bootable 160 KiB floppy (no DOS) | `link.ld` | `runtime/crt0.s` | boot sector at 0000:7C00 with the 55AAh signature; stage 2 read to 0000:7E00 by INT 13h; `.bss` at 0500h..; stack below 7C00h | `i8086-ld --sign --floppy` |
| DOS `.COM` | `link_com.ld` | `runtime/crt0_com.s` | CS:0100h, one 64 KiB segment, stack from the top of memory in the PSP (word at 0002h) | `i8086-ld --com` |
| DOS `.EXE`, one or several segments | `link_com.ld` (tiny model) or `link_exe_seg.ld` | `crt0_com.s`, or a program entry | MZ header of one sector; segments after the first start at the first paragraph past the previous contents; each `sym@seg` is in the relocation table; `__stack_top` gives the header's SP | `i8086-ld --exe` |

Memory checks are made by the linker (a region overflow is an error that names the overflow in bytes) and again by `scripts/check_images.py` against the map: nothing below 0500h, nothing at or above the top of the 64 KiB reference RAM (10000h) except the CGA page, the boot floppy within 160 KiB, `.COM` within 64,000 bytes.

### Interrupts and vectors

The IVT is at 0000:0000, four bytes a vector (offset, segment). A handler is a Rust `extern "msp430-interrupt" fn`; `ibm_pc_hal::set_vector(n, handler)` stores it (with interrupts masked) and `cli`/`sti` are `hal_cli`/`hal_sti`. Hardware interrupts need the 8259 acknowledged by the handler (`hal_outb(0x20, 0x20)`); the timer (vector 8) and keyboard (vector 9) are the ones the smoke program takes over.

### Runtime and HAL contract

- `platform/runtime/crt0.s`, `crt0_com.s`, `dos.s`, `hal.s`: reset or entry, stack, `.bss`, DOS calls, BIOS teletype, port I/O, far memory access, `cli`/`sti`, vector installation;
- `platform/hal` (`ibm_pc_hal`, `no_std`): Rust names for the HAL and a `core::fmt::Write` console; `platform/dos` (`ibm_pc_dos`): `Regs`, `int21`, `println!`, `exit`;
- the application or OS rewrite (the software issue [ai_engineer#81](https://github.com/hevangel/ai_engineer/issues/81)) is not part of this profile.

## Legal firmware and media

The IBM BIOS (`bios.bin`), Cassette BASIC and PC DOS 1.00 are copyrighted and are **not** in the repository. `scripts/` and the tests take them from `$AI_HW_ENGINEER/system/ibm_pc/work/reference` and check their SHA-256 (BIOS `a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8`, BASIC `2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102`, DOS disk `29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2`) before use. Images built here contain only code compiled from this repository; a DOS disk with a program on it is made on the fly with `pcdisk` and never committed.
