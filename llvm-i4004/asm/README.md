# llvm-i4004 assembler / linker

Assembler: the LLVM MC assembler and `llvm-mc` built by `scripts/build_llvm.sh`
(target `i4004`, ELF `e_machine=0x4004`). There is no upstream `ld.lld` hook for
the i4004, and the MCS-4 image is a flat ROM with a fixed five-chip page layout,
so linking/image-packaging is done by a small standalone tool here.

## `i4004_pack.py` — image packer / linker

Takes one or more ELF32-LSB relocatable objects from `llvm-mc`, lays their
sections out per `system/busicom_141pf/platform/link.ld`, resolves the
`R_I4004_*` relocations (abi.md §5.1), enforces the page-local constraint, and
writes a raw ROM `.bin` plus a `.map`. It parses ELF by hand (no `pyelftools`
dependency) so it runs in the bare container.

```
i4004_pack.py OBJ... -o IMAGE.bin [-m IMAGE.map] [--max N] [--pad N]
              [--pad-pages] [--allow-eop-rollforward] [--linker-script link.ld]
```

- `--max N` — ROM budget in bytes (default 1280, the five-4001 Busicom board;
  up to 4096, the architectural max). Over-budget and `--max > 4096` are errors.
- `--pad N` — pad the output `.bin` to N bytes (e.g. `--pad 1280` for a board
  image). The runner accepts 1280–4096.
- `--pad-pages` — keep each **code** section whole inside one 256-byte 4001
  page (bump to the next page rather than split). Guarantees the page-local
  `JCN`/`ISZ`/`FIN` branches inside a function never cross a page. A single
  section larger than 256 bytes is an error (split the function).
- `--allow-eop-rollforward` — permit a page-local op whose second word is
  fetched across a page boundary (the isa.md §9 end-of-page roll-forward).
  Default: reject it.

### Relocations resolved

`R_I4004_ADDR12` (JUN/JMS, A3 in byte-0 low nibble + A2A1 in byte 1),
`R_I4004_PAGE8`/`R_I4004_FIN8` (JCN/ISZ/FIN second word; **page-local — a
page-crossing target is a hard error** naming both pages, unless `--pad-pages`
relocated it onto the page or you intend the roll-forward), `R_I4004_IMM8`
(FIM), `R_I4004_ROMADDR12`/`R_I4004_RAMADDR16` (pointers stored as data). RELA
addends; cross-section references resolve as `section_base + addend`.

### Linker-defined symbols

`__sstack_base`, `__sstack_top`, `__data_load`, `__data_start`, `__data_end`,
`__data_size`, `__bss_start`, `__bss_end`, `__bss_size` — consumed by `crt0.s`.

### Section placement (link.ld)

`.reset` at ROM 0x000, then `.text`, `.rodata`. `.data` gets a ROM load image
(appended after code) plus a RAM run address; `.bss` is RAM-only (NOLOAD). RAM
addresses are the 16-bit AS-0 handle (abi.md §1.3); for the Busicom board the
low byte is the SRC address. Only the ROM image is written to the `.bin`.

### Example

```
llvm-mc -triple i4004 --filetype=obj crt0.s  -o crt0.o
llvm-mc -triple i4004 --filetype=obj hal.s   -o hal.o
llvm-mc -triple i4004 --filetype=obj main.s  -o main.o
i4004_pack.py crt0.o hal.o main.o -o image.bin --pad-pages --pad 1280 -m image.map
```

When `ld.lld` grows a working i4004 page-aware placement, this tool becomes the
fallback; until then it IS the linker. `scripts/build_image.sh` is its intended
driver contract (Rust → IR → `llc` → `llvm-mc` → `i4004_pack.py`).
