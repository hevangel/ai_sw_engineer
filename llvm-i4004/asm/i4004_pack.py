#!/usr/bin/env python3
"""i4004 image packer / linker.

Takes one or more ELF32-LSB relocatable objects produced by our ``llvm-mc``
(``e_machine = 0x4004``), lays their sections out in the two MCS-4 ROM/RAM
spaces per ``system/busicom_141pf/platform/link.ld``, resolves the
``R_I4004_*`` relocations (abi.md sec.5.1), enforces the page-local constraint
for ``PAGE8``/``FIN8`` targets, and writes a raw ROM ``.bin`` plus a ``.map``.

This is a standalone linker: ``ld.lld`` has no i4004 hook, and the MCS-4 image
is a flat ROM with a fixed five-chip page layout, so a small deterministic
packer is cheaper and fully under our control. It parses ELF by hand (no
pyelftools dependency) because ELF32-LSB relocatable objects are simple and the
tool must be committable and run in the bare container.

Layout model (link.ld):
  * ROM (code/rodata): byte address, 256-byte 4001 pages, chip select =
    addr[11:8]. Budget 1280 bytes (0x000-0x4FF) for the stock board, 4096
    (0x000-0xFFF) with --max 4096.
  * The reset section (``.reset``) is placed first at ROM 0x000.
  * ``.text`` and ``.rodata`` follow in ROM.
  * ``.data`` has a ROM load image (copied to RAM by crt0) AND a RAM run
    address; ``.bss`` is RAM-only (NOLOAD). RAM addresses are the backend's
    16-bit AS-0 handle (abi.md sec.1.3) and are NOT part of the ROM .bin.

Page-local policy (abi.md sec.5.1, isa.md sec.9):
  A ``PAGE8``/``FIN8`` relocation only writes the low 8 bits of the address; the
  high nibble (page) is taken from the PC. The referent must therefore lie on
  the SAME 256-byte page as the *word that consumes the second byte*. By
  default the packer REJECTS a page-crossing PAGE8/FIN8 (hard error with both
  addresses and pages named). With --pad-pages it instead pads functions so
  each input ``.text``/``.reset`` section is placed whole inside a single page
  (never split across a page boundary); a section larger than 256 bytes is
  still an error. The end-of-page quirk (a 2-word page-local op whose opcode is
  at offset 0xFF) is also rejected unless --allow-eop-rollforward is given.
"""

from __future__ import annotations

import argparse
import struct
import sys
from dataclasses import dataclass, field

PAGE = 0x100

# e_machine and relocation numbers -- must match the MC backend
# (I4004ELFObjectWriter.cpp / ELFRelocs/I4004.def).
EM_I4004 = 0x4004
R_I4004_NONE = 0
R_I4004_ADDR12 = 1
R_I4004_PAGE8 = 2
R_I4004_IMM8 = 3
R_I4004_FIN8 = 4
R_I4004_ROMADDR12 = 5
R_I4004_RAMADDR16 = 6

RELOC_NAME = {
    R_I4004_NONE: "R_I4004_NONE",
    R_I4004_ADDR12: "R_I4004_ADDR12",
    R_I4004_PAGE8: "R_I4004_PAGE8",
    R_I4004_IMM8: "R_I4004_IMM8",
    R_I4004_FIN8: "R_I4004_FIN8",
    R_I4004_ROMADDR12: "R_I4004_ROMADDR12",
    R_I4004_RAMADDR16: "R_I4004_RAMADDR16",
}

# ELF section-header types / flags we care about
SHT_PROGBITS = 1
SHT_SYMTAB = 2
SHT_STRTAB = 3
SHT_RELA = 4
SHT_NOBITS = 8
SHF_ALLOC = 0x2

# ROM vs RAM classification by section name prefix.
ROM_SECTIONS = (".reset", ".text", ".rodata")
RAM_LOAD_SECTIONS = (".data",)       # ROM-loaded, RAM-resident (crt0 copies)
RAM_NOLOAD_SECTIONS = (".bss", ".sstack", "COMMON")


class LinkError(Exception):
    pass


# --------------------------------------------------------------------------
# minimal ELF32-LSB reader
# --------------------------------------------------------------------------
@dataclass
class Section:
    name: str
    sh_type: int
    flags: int
    addr: int
    offset: int
    size: int
    link: int
    info: int
    addralign: int
    entsize: int
    index: int
    data: bytes = b""
    # assigned by the linker:
    base: int = -1          # final address (ROM byte addr or RAM handle)
    rom_load: int = -1      # for .data: where its load image sits in ROM


@dataclass
class Symbol:
    name: str
    value: int
    size: int
    info: int
    shndx: int
    # resolved:
    section_index: int = -1


@dataclass
class Reloc:
    offset: int
    sym: int
    rtype: int
    addend: int


@dataclass
class ObjFile:
    path: str
    sections: list = field(default_factory=list)
    symbols: list = field(default_factory=list)      # from .symtab
    relocs: dict = field(default_factory=dict)        # target_sec_index -> [Reloc]


def _cstr(buf: bytes, off: int) -> str:
    end = buf.find(b"\0", off)
    return buf[off:end].decode("ascii", "replace")


def read_elf(path: str) -> ObjFile:
    with open(path, "rb") as fh:
        data = fh.read()
    if data[:4] != b"\x7fELF":
        raise LinkError(f"{path}: not an ELF file")
    ei_class, ei_data = data[4], data[5]
    if ei_class != 1 or ei_data != 1:
        raise LinkError(f"{path}: expected ELFCLASS32/ELFDATA2LSB")
    (e_type, e_machine, e_version, e_entry, e_phoff, e_shoff, e_flags,
     e_ehsize, e_phentsize, e_phnum, e_shentsize, e_shnum,
     e_shstrndx) = struct.unpack_from("<HHIIIIIHHHHHH", data, 16)
    if e_machine != EM_I4004:
        raise LinkError(f"{path}: e_machine 0x{e_machine:x} != 0x{EM_I4004:x}")

    # section headers
    raw_sections = []
    for i in range(e_shnum):
        off = e_shoff + i * e_shentsize
        (sh_name, sh_type, sh_flags, sh_addr, sh_offset, sh_size, sh_link,
         sh_info, sh_addralign, sh_entsize) = struct.unpack_from(
            "<IIIIIIIIII", data, off)
        raw_sections.append((sh_name, sh_type, sh_flags, sh_addr, sh_offset,
                             sh_size, sh_link, sh_info, sh_addralign,
                             sh_entsize))

    shstr_off = raw_sections[e_shstrndx][4]
    shstr_size = raw_sections[e_shstrndx][5]
    shstrtab = data[shstr_off:shstr_off + shstr_size]

    sections = []
    for i, r in enumerate(raw_sections):
        (sh_name, sh_type, sh_flags, sh_addr, sh_offset, sh_size, sh_link,
         sh_info, sh_addralign, sh_entsize) = r
        name = _cstr(shstrtab, sh_name)
        sdata = b"" if sh_type == SHT_NOBITS else data[sh_offset:sh_offset + sh_size]
        sections.append(Section(name, sh_type, sh_flags, sh_addr, sh_offset,
                                 sh_size, sh_link, sh_info, sh_addralign,
                                 sh_entsize, i, sdata))

    obj = ObjFile(path=path, sections=sections)

    # symbol table + its string table
    symtab = next((s for s in sections if s.sh_type == SHT_SYMTAB), None)
    if symtab is not None:
        strtab = sections[symtab.link].data
        nsym = symtab.size // 16
        for k in range(nsym):
            st_name, st_value, st_size, st_info, st_other, st_shndx = \
                struct.unpack_from("<IIIBBH", symtab.data, k * 16)
            obj.symbols.append(
                Symbol(_cstr(strtab, st_name), st_value, st_size, st_info,
                       st_shndx))

    # relocations (RELA only -- our writer sets HasRelocationAddend=true)
    for s in sections:
        if s.sh_type != SHT_RELA:
            continue
        target = s.info     # section the relocations apply to
        rels = []
        n = s.size // 12
        for k in range(n):
            r_offset, r_info, r_addend = struct.unpack_from("<IIi", s.data, k * 12)
            sym = r_info >> 8
            rtype = r_info & 0xff
            rels.append(Reloc(r_offset, sym, rtype, r_addend))
        obj.relocs.setdefault(target, []).extend(rels)

    return obj


# --------------------------------------------------------------------------
# linker
# --------------------------------------------------------------------------
@dataclass
class Placed:
    """An input section placed into the output image."""
    obj: ObjFile
    sec: Section
    base: int          # final address (ROM byte or RAM handle)
    rom_load: int      # ROM load addr for .data, else -1
    kind: str          # 'rom' | 'ram_load' | 'ram_noload'


def classify(name: str) -> str:
    base = name.split(".", 2)
    # match on the leading component, e.g. .text.foo -> .text
    for pfx in ROM_SECTIONS:
        if name == pfx or name.startswith(pfx + "."):
            return "rom"
    for pfx in RAM_LOAD_SECTIONS:
        if name == pfx or name.startswith(pfx + "."):
            return "ram_load"
    for pfx in RAM_NOLOAD_SECTIONS:
        if name == pfx or name.startswith(pfx + "."):
            return "ram_noload"
    return "ignore"


class Linker:
    def __init__(self, objs, rom_len, ram_origin, ram_len,
                 stack_origin, stack_len, pad_pages, allow_eop):
        self.objs = objs
        self.rom_len = rom_len
        self.ram_origin = ram_origin
        self.ram_len = ram_len
        self.stack_origin = stack_origin
        self.stack_len = stack_len
        self.pad_pages = pad_pages
        self.allow_eop = allow_eop
        self.placed: list[Placed] = []
        self.rom_image = bytearray()
        self.global_syms: dict[str, tuple[ObjFile, Symbol]] = {}
        self.linker_syms: dict[str, int] = {}
        self.warnings: list[str] = []

    # -- symbol resolution --------------------------------------------------
    def _collect_globals(self):
        STB_GLOBAL = 1
        for obj in self.objs:
            for sym in obj.symbols:
                bind = sym.info >> 4
                if bind == STB_GLOBAL and sym.shndx not in (0,):  # defined
                    if sym.name in self.global_syms:
                        raise LinkError(f"duplicate global symbol '{sym.name}'")
                    self.global_syms[sym.name] = (obj, sym)

    # -- section placement --------------------------------------------------
    def _ordered_rom_inputs(self):
        """Return ROM input sections, .reset first, then .text, then .rodata."""
        buckets = {".reset": [], ".text": [], ".rodata": []}
        for obj in self.objs:
            for sec in obj.sections:
                if classify(sec.name) != "rom":
                    continue
                for pfx in (".reset", ".text", ".rodata"):
                    if sec.name == pfx or sec.name.startswith(pfx + "."):
                        buckets[pfx].append((obj, sec))
                        break
        return buckets[".reset"] + buckets[".text"] + buckets[".rodata"]

    def _place_rom(self):
        addr = 0
        for obj, sec in self._ordered_rom_inputs():
            align = max(sec.addralign, 1)
            addr = (addr + align - 1) & ~(align - 1)
            is_code = not (sec.name == ".rodata" or sec.name.startswith(".rodata"))
            if self.pad_pages and is_code and sec.size > 0:
                # keep a code section whole inside one 256-byte page
                if sec.size > PAGE:
                    raise LinkError(
                        f"{obj.path}:{sec.name} is {sec.size} bytes > one 256-byte "
                        f"page; cannot keep it page-local (split the function)")
                if (addr // PAGE) != ((addr + sec.size - 1) // PAGE):
                    addr = (addr + PAGE - 1) & ~(PAGE - 1)  # bump to next page
            sec.base = addr
            self.placed.append(Placed(obj, sec, addr, -1, "rom"))
            addr += sec.size
        self.rom_used = addr
        if addr > self.rom_len:
            raise LinkError(
                f"ROM image {addr} bytes exceeds budget {self.rom_len} "
                f"(0x{self.rom_len:x}); add 4001s / raise --max")

    def _place_ram(self):
        # .data: RAM run address (AS-0 handle) + ROM load image appended.
        addr = self.ram_origin
        rom_load = self.rom_used
        for obj, sec in [(o, s) for o in self.objs for s in o.sections
                         if classify(s.name) == "ram_load"]:
            align = max(sec.addralign, 1)
            addr = (addr + align - 1) & ~(align - 1)
            sec.base = addr
            sec.rom_load = rom_load
            self.placed.append(Placed(obj, sec, addr, rom_load, "ram_load"))
            addr += sec.size
            rom_load += sec.size
        self.data_rom_end = rom_load
        # .bss / COMMON: RAM only
        for obj, sec in [(o, s) for o in self.objs for s in o.sections
                         if classify(s.name) == "ram_noload"
                         and not (s.name == ".sstack" or s.name.startswith(".sstack"))]:
            align = max(sec.addralign, 1)
            addr = (addr + align - 1) & ~(align - 1)
            sec.base = addr
            self.placed.append(Placed(obj, sec, addr, -1, "ram_noload"))
            addr += sec.size
        self.ram_used = addr - self.ram_origin
        if addr > self.ram_origin + self.ram_len:
            raise LinkError(
                f"RAM .data/.bss {self.ram_used} nibbles exceeds arena "
                f"{self.ram_len} (0x{self.ram_origin:x}..0x{self.ram_origin+self.ram_len:x})")
        if self.data_rom_end > self.rom_len:
            raise LinkError(
                f".data ROM load image overflows ROM budget ({self.data_rom_end} > {self.rom_len})")

    def _linker_defined(self):
        self.linker_syms["__sstack_base"] = self.stack_origin
        self.linker_syms["__sstack_top"] = self.stack_origin + self.stack_len
        # __data_load / __data_start / __data_end / __bss_start / __bss_end
        data_run_start = None
        data_run_end = None
        data_load = None
        bss_start = None
        bss_end = None
        for p in self.placed:
            if p.kind == "ram_load":
                if data_run_start is None:
                    data_run_start = p.base
                    data_load = p.rom_load
                data_run_end = p.base + p.sec.size
            elif p.kind == "ram_noload":
                if bss_start is None:
                    bss_start = p.base
                bss_end = p.base + p.sec.size
        self.linker_syms["__data_load"] = data_load if data_load is not None else self.data_rom_end
        self.linker_syms["__data_start"] = data_run_start if data_run_start is not None else self.ram_origin
        self.linker_syms["__data_end"] = data_run_end if data_run_end is not None else self.ram_origin
        self.linker_syms["__bss_start"] = bss_start if bss_start is not None else (data_run_end or self.ram_origin)
        self.linker_syms["__bss_end"] = bss_end if bss_end is not None else (data_run_end or self.ram_origin)
        # byte-count sizes for crt0 count-down loops (nibbles in RAM / bytes in ROM)
        self.linker_syms["__data_size"] = self.linker_syms["__data_end"] - self.linker_syms["__data_start"]
        self.linker_syms["__bss_size"] = self.linker_syms["__bss_end"] - self.linker_syms["__bss_start"]

    # -- relocation resolution ---------------------------------------------
    def _sym_addr(self, obj: ObjFile, rel: Reloc) -> int:
        """Address S for a relocation's symbol (local, global, or section)."""
        sym = obj.symbols[rel.sym]
        STT_SECTION = 3
        if (sym.info & 0xf) == STT_SECTION or sym.name == "":
            # reference is to a section symbol: S = that section's placed base
            sec = obj.sections[sym.shndx]
            if sec.base < 0:
                raise LinkError(f"{obj.path}: reloc targets unplaced section "
                                f"{sec.name}")
            return sec.base
        if sym.shndx == 0:  # undefined -> must be global or linker-defined
            if sym.name in self.linker_syms:
                return self.linker_syms[sym.name]
            if sym.name in self.global_syms:
                g_obj, g_sym = self.global_syms[sym.name]
                g_sec = g_obj.sections[g_sym.shndx]
                return g_sec.base + g_sym.value
            raise LinkError(f"{obj.path}: undefined symbol '{sym.name}'")
        # defined local/global in this object
        sec = obj.sections[sym.shndx]
        if sec.base < 0:
            raise LinkError(f"{obj.path}: reloc targets unplaced section {sec.name}")
        return sec.base + sym.value

    def _apply(self):
        # build a flat ROM image (reset+text+rodata+data load image)
        self.rom_image = bytearray(self.data_rom_end)
        for p in self.placed:
            if p.kind == "rom":
                self.rom_image[p.base:p.base + p.sec.size] = p.sec.data
            elif p.kind == "ram_load":
                self.rom_image[p.rom_load:p.rom_load + p.sec.size] = p.sec.data

        for obj in self.objs:
            for sec in obj.sections:
                rels = obj.relocs.get(sec.index)
                if not rels:
                    continue
                if sec.base < 0:
                    # relocations in a non-placed section (shouldn't happen)
                    continue
                # where does this section live in the ROM image?
                if classify(sec.name) == "rom":
                    img_base = sec.base
                elif classify(sec.name) == "ram_load":
                    img_base = sec.rom_load
                else:
                    # relocations into .bss etc. have no file bytes to patch
                    continue
                for rel in rels:
                    S = self._sym_addr(obj, rel)
                    value = S + rel.addend
                    self._patch(obj, sec, img_base, rel, value)

    def _patch(self, obj, sec, img_base, rel, value):
        off = img_base + rel.offset
        rt = rel.rtype
        if rt == R_I4004_NONE:
            return
        if rt == R_I4004_ADDR12:
            # byte0 low nibble = A3 (keep opcode high nibble), byte1 = A2A1
            if value > 0xFFF:
                raise LinkError(self._loc(obj, sec, rel) +
                                f": ADDR12 target 0x{value:x} exceeds 12 bits")
            op = self.rom_image[off] & 0xF0
            self.rom_image[off] = op | ((value >> 8) & 0x0F)
            self.rom_image[off + 1] = value & 0xFF
        elif rt in (R_I4004_PAGE8, R_I4004_FIN8):
            # the second word sits at off+? -- for PAGE8/FIN8 the reloc offset
            # already points at the word that holds the 8-bit field.
            consumer_page = (img_base + rel.offset) // PAGE
            target_page = value // PAGE
            # end-of-page roll-forward: a page-local op whose second word is
            # fetched from offset 0x00 of the next page advances PC[11:8].
            if (img_base + rel.offset) % PAGE == 0x00 and rel.offset >= 1:
                # the opcode was at 0xFF of the previous page; PC already rolled
                consumer_page = (img_base + rel.offset) // PAGE
                if not self.allow_eop:
                    raise LinkError(self._loc(obj, sec, rel) +
                        f": page-local {RELOC_NAME[rt]} second word at a page "
                        f"boundary (end-of-page roll-forward); pass "
                        f"--allow-eop-rollforward if intended")
            if target_page != consumer_page:
                raise LinkError(self._loc(obj, sec, rel) +
                    f": {RELOC_NAME[rt]} target 0x{value:03x} (page {target_page}) "
                    f"is not on the consuming word's page {consumer_page} "
                    f"(addr 0x{(img_base+rel.offset):03x}); page-crossing "
                    f"page-local relocation is illegal (abi.md 5.1). "
                    f"Use --pad-pages to keep callers/targets page-local, or a "
                    f"JUN trampoline.")
            self.rom_image[off] = value & 0xFF
        elif rt == R_I4004_IMM8:
            self.rom_image[off] = value & 0xFF
        elif rt == R_I4004_ROMADDR12:
            # 12-bit ROM address stored zero-extended into a 2-byte data cell
            self.rom_image[off] = value & 0xFF
            self.rom_image[off + 1] = (value >> 8) & 0x0F
        elif rt == R_I4004_RAMADDR16:
            self.rom_image[off] = value & 0xFF
            self.rom_image[off + 1] = (value >> 8) & 0xFF
        else:
            raise LinkError(self._loc(obj, sec, rel) +
                            f": unhandled relocation type {rt}")

    def _loc(self, obj, sec, rel):
        return f"{obj.path}:{sec.name}+0x{rel.offset:x}"

    # -- driver -------------------------------------------------------------
    def link(self):
        self._collect_globals()
        self._place_rom()
        self._place_ram()
        self._linker_defined()
        self._apply()

    def rom_bin(self, pad_to=None):
        out = bytearray(self.rom_image)
        if pad_to is not None and len(out) < pad_to:
            out.extend(b"\x00" * (pad_to - len(out)))
        return bytes(out)

    def mapfile(self) -> str:
        lines = []
        lines.append("i4004 image map")
        lines.append("=" * 60)
        lines.append(f"ROM used : {self.rom_used} bytes "
                     f"(0x000-0x{max(self.rom_used-1,0):03x}) of {self.rom_len}")
        lines.append(f"  .data load image: 0x{self.rom_used:03x}-0x{max(self.data_rom_end-1,0):03x} "
                     f"({self.data_rom_end-self.rom_used} bytes)" if self.data_rom_end > self.rom_used
                     else "  .data load image: (none)")
        lines.append(f"RAM used : {self.ram_used} nibbles of {self.ram_len}")
        lines.append("")
        lines.append("Sections (final placement):")
        for p in sorted(self.placed, key=lambda q: (q.kind != "rom", q.base)):
            if p.kind == "rom":
                lines.append(f"  ROM  0x{p.base:03x} +{p.sec.size:<4} {p.obj.path.split('/')[-1]}:{p.sec.name}")
            elif p.kind == "ram_load":
                lines.append(f"  RAM  0x{p.base:03x} +{p.sec.size:<4} {p.obj.path.split('/')[-1]}:{p.sec.name} "
                             f"(load@ROM 0x{p.rom_load:03x})")
            else:
                lines.append(f"  RAM  0x{p.base:03x} +{p.sec.size:<4} {p.obj.path.split('/')[-1]}:{p.sec.name} (nobits)")
        lines.append("")
        lines.append("Linker-defined symbols:")
        for k, v in sorted(self.linker_syms.items()):
            lines.append(f"  {k:16} = 0x{v:03x}")
        lines.append("")
        lines.append("Global symbols:")
        for name in sorted(self.global_syms):
            g_obj, g_sym = self.global_syms[name]
            sec = g_obj.sections[g_sym.shndx]
            lines.append(f"  {name:20} = 0x{sec.base + g_sym.value:03x}  ({sec.name})")
        return "\n".join(lines) + "\n"


def parse_memory_from_ld(path):
    """Best-effort pull of ROM/RAM/stack ORIGIN/LENGTH from link.ld.

    The packer defaults match link.ld; this lets --linker-script override them
    without re-parsing GNU-ld grammar fully. Only MEMORY ORIGIN/LENGTH lines of
    the form `name (..) : ORIGIN = X, LENGTH = Y` are read."""
    import re
    regions = {}
    with open(path) as fh:
        for line in fh:
            m = re.match(r"\s*(\w+)\s*\([^)]*\)\s*:\s*ORIGIN\s*=\s*(0x[0-9a-fA-F]+|\d+)\s*,\s*LENGTH\s*=\s*(0x[0-9a-fA-F]+|\d+)", line)
            if m:
                regions[m.group(1)] = (int(m.group(2), 0), int(m.group(3), 0))
    return regions


def main(argv=None):
    ap = argparse.ArgumentParser(description="i4004 ELF -> ROM image packer/linker")
    ap.add_argument("objects", nargs="+", help="ELF32 relocatable objects (from llvm-mc)")
    ap.add_argument("-o", "--output", required=True, help="output ROM .bin")
    ap.add_argument("-m", "--map", dest="mapfile", help="output .map file")
    ap.add_argument("--max", type=lambda s: int(s, 0), default=1280,
                    help="ROM budget bytes (1280 stock board, up to 4096)")
    ap.add_argument("--pad", type=lambda s: int(s, 0), default=None,
                    help="pad the .bin to this size (e.g. 1280 for the stock board)")
    ap.add_argument("--pad-pages", action="store_true",
                    help="keep each code section whole inside one 256-byte page")
    ap.add_argument("--allow-eop-rollforward", action="store_true",
                    help="permit a page-local op whose 2nd word crosses a page boundary")
    ap.add_argument("--linker-script", help="read ROM/RAM/stack regions from a link.ld")
    args = ap.parse_args(argv)

    rom_len = args.max
    ram_origin, ram_len = 0x0000, 0x0040
    stack_origin, stack_len = 0x0040, 0x0040
    if args.linker_script:
        regions = parse_memory_from_ld(args.linker_script)
        if "rom" in regions:
            rom_len = min(args.max, regions["rom"][1]) if args.max else regions["rom"][1]
            rom_len = regions["rom"][1] if args.max == 1280 else args.max
        if "ram" in regions:
            ram_origin, ram_len = regions["ram"]
        if "stack" in regions:
            stack_origin, stack_len = regions["stack"]

    if rom_len > 4096:
        print(f"ERROR: ROM budget {rom_len} exceeds 4004 architectural max 4096",
              file=sys.stderr)
        return 2

    try:
        objs = [read_elf(p) for p in args.objects]
        ln = Linker(objs, rom_len, ram_origin, ram_len, stack_origin, stack_len,
                    args.pad_pages, args.allow_eop_rollforward)
        ln.link()
        pad_to = args.pad
        binimg = ln.rom_bin(pad_to=pad_to)
        with open(args.output, "wb") as fh:
            fh.write(binimg)
        if args.mapfile:
            with open(args.mapfile, "w") as fh:
                fh.write(ln.mapfile())
        print(f"wrote {args.output}: {len(binimg)} bytes "
              f"(code+rodata {ln.rom_used}, data-load {ln.data_rom_end - ln.rom_used}, "
              f"RAM {ln.ram_used} nibbles)", file=sys.stderr)
    except LinkError as e:
        print(f"i4004_pack: link error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
