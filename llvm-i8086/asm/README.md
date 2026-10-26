# i8086-ld: the llvm-i8086 linker and image packager

A standalone Rust tool (no dependencies) instead of `ld.lld`: LLD has no `EM_I8086` target and the platform needs a packager for a bootable floppy, so the same precedent as `llvm-i8008/asm` applies. It reads the ELF32 objects that `llc -filetype=obj` and `llvm-mc` write, and does what a title needs: place sections into the memory regions of a linker script, drop what nothing uses, apply the relocations, check the sizes, and write the image.

```text
i8086-ld -T system/ibm_pc/platform/link.ld --sign \
         -o title.elf --bin title.bin --floppy title.img -Map title.map  a.o b.o rt/obj/*.o
```

`scripts/link_image.sh` builds the tool and runs it; `scripts/build_image.sh` also compiles `.s` and `.ll` inputs and adds the runtime library.

## What it does

1. **Symbols.** Global and weak definitions across all objects; a strong definition beats a weak one, two strong ones are an error naming both objects. A reference to a global symbol resolves by name even when its own object has a weak definition. Weak undefined is address 0. Common symbols are rejected.
2. **Placement.** Each allocatable input section goes to the first output section of the script whose rule matches (file glob, section glob), in script order; inside an output section by rule order, then object order, then section order. Alignment is the section's; an output section is aligned to its most aligned input. An output section marked `(NOLOAD)`, or made only of NOBITS inputs, takes addresses but no bytes (bss). A section no rule matches is an error; `/DISCARD/` drops sections silently.
3. **Memory regions.** A region that overflows is an error saying by how many bytes. Regions must lie inside the 64 KiB segment (the tiny model: every address is both an offset and the physical address).
4. **Section garbage collection** (default; `--no-gc-sections` turns it off). Roots: `KEEP(...)` sections, the entry symbol (`-e`, the script's `ENTRY`, else `_start` if present), and `-u` symbols. A section is kept if a kept section has a relocation to a symbol in it. Everything else is listed under "Discarded" in the map. A reference to an undefined symbol from a kept section is an error listing every undefined name and the first object that refers to it; references from dropped sections are not errors.
5. **Relocation** (spec/abi.md): `R_I8086_8` and `R_I8086_16` store `S + A` (range -128..255 and -32768..65535), `R_I8086_PC8` stores `S + A - P` and must lie in -128..127 (the message names the object, section and address), `R_I8086_PC16` stores `S + A - P` modulo 2^16 (a near jump or call reaches anywhere in the segment).
6. **Script symbols.** `sym = value;` and `PROVIDE(sym = value);` from the script, and for every output section `__<name>_start` and `__<name>_end` (`.bss` gives `__bss_start`, `__bss_end`, which a crt0 uses to clear bss).

## Outputs

- `-o x.elf`: an `ET_EXEC` file with `EM_I8086`, a `PT_LOAD` for each output section, section headers and a symbol table, so `llvm-objdump -d x.elf` disassembles it with labels (an independent decoder for the result).
- `--bin x.bin`: the loaded sections from the lowest address to the highest, gaps zero-filled.
- `--floppy x.img`: a bootable 160 KiB disk (40 tracks x 8 sectors x 512 bytes, the 5150's single-sided format). The first 512 bytes are the boot sector, which the BIOS loads at `--boot-addr` (default `0x7C00`) and which must end with `55 AA`. `--sign` writes the signature if the sector has zeros there and refuses to overwrite code. The rest of the image follows sector after sector, which is what INT 13h reads. The image starts at the boot address or it is an error.
- `-Map x.map`: regions used, output and input sections with addresses and sizes, symbols by address, and the discarded sections.

## Linker script subset

`ENTRY`, `MEMORY` (`ORIGIN`, `LENGTH`, attributes ignored), `SECTIONS` with `name [(NOLOAD)] : { [KEEP(]file(globs...)[)] ... } > region`, `/DISCARD/`, assignments and `PROVIDE`. Expressions: numbers (`0x`, `K`, `M`), `ORIGIN(r)`, `LENGTH(r)`, earlier symbols, `+`, `-`. Anything else (the location counter, `AT>`, `ALIGN`, `SORT`, `INCLUDE`) is an error with its line number, not silently ignored. The platform scripts in `system/*/platform/link.ld` are inside this subset (`tests/link.rs` parses the IBM PC one).

## Not supported

Archives (`.a`), shared objects, `-r` partial links, common symbols, linker relaxation (the assembler already relaxes branches within a section, so a branch to another section is the long form), far (segment:offset) relocations, overlays and several segments (tiny model only), debug-section processing (non-allocatable sections are ignored).

## Tests

`cargo test` here: 19 tests on objects built in the test (`tests/common`): placement order and alignment, each relocation kind and its range, garbage collection, weak and duplicate and undefined symbols, region overflow, bss, the ELF output, `/DISCARD/`, the floppy checks, script errors with line numbers, the IBM PC script. `scripts/run_tests.sh` links `tests/boot/hello.o` with the platform script and checks the `llvm-objdump` disassembly with FileCheck. The emulator tests link every compiled test program and the runtime library with it (`system/ibm_pc/emulator/tests/i8086_codegen.rs`) and boot a linked floppy image through the original BIOS (`i8086_boot.rs`).
