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
import os
import struct
import sys
from dataclasses import dataclass, field

PAGE = 0x100

# Software-call return dispatch (abi.md 4.4.1/4.4.2). A one-byte `jin p0`
# (opcode 0x31) is reserved at STUB_OFF on EVERY ROM page; __soft_ret pops the
# 3-nibble return token and dispatches through a per-page JUN table (__srtab)
# to that page's stub, which `jin p0`s to the return offset.
STUB_OFF = 0xFC          # fixed per-page stub offset (1 byte: jin p0)
JIN_P0 = 0x31            # `jin p0` encoding (verified, abi.md 4.4.1)
SOFT_RET_SYM = "__soft_ret"
SRTAB_SYM = "__srtab"
PUSH_OFF_SYM = "__push_off"
PUSH_PG_SYM = "__push_pg"
PUSH_PB_SYM = "__push_pb"
PUSH1_SYM = "__push1"
SAVE_P4P5_SYM = "__save_p4p5"

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
R_I4004_LO8 = 7
R_I4004_PAGE4 = 8
# Bank nibble (the ROM bank a code symbol was placed in by the packer) into the
# low nibble of an LDM operand byte (far-call forward token, abi.md sec.4.4.3).
# The packer is the ONLY producer: it fills the nibble from its bank assignment
# when it synthesizes the far-call sequence, so the backend never emits it from
# a .ll (bank numbers do not exist until link-time placement). From %bank(sym).
R_I4004_BANK4 = 9

RELOC_NAME = {
    R_I4004_NONE: "R_I4004_NONE",
    R_I4004_ADDR12: "R_I4004_ADDR12",
    R_I4004_PAGE8: "R_I4004_PAGE8",
    R_I4004_IMM8: "R_I4004_IMM8",
    R_I4004_FIN8: "R_I4004_FIN8",
    R_I4004_ROMADDR12: "R_I4004_ROMADDR12",
    R_I4004_RAMADDR16: "R_I4004_RAMADDR16",
    R_I4004_LO8: "R_I4004_LO8",
    R_I4004_PAGE4: "R_I4004_PAGE4",
    R_I4004_BANK4: "R_I4004_BANK4",
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
                 stack_origin, stack_len, pad_pages, allow_eop,
                 banks=0, common_bytes=0x400, bank_bytes=0xC00):
        self.objs = objs
        self.rom_len = rom_len
        self.ram_origin = ram_origin
        self.ram_len = ram_len
        self.stack_origin = stack_origin
        self.stack_len = stack_len
        self.pad_pages = pad_pages
        self.allow_eop = allow_eop
        # Banking (item 2): banks>0 enables the xl bank-switched layout.
        #   common region = ROM chips #0..#3 (common_bytes, default 0x400), the
        #     same in every bank; holds .reset/crt0/HAL/hot-rt + trampolines.
        #   banked region = chips #4..#15 (bank_bytes, default 0xC00) per bank.
        # Address of a banked function is 0x400 + offset-in-bank (same PC in
        # every bank, different physical content). Cross-bank calls route
        # through a __bankcall_* trampoline in the common region.
        self.banks = banks
        self.common_bytes = common_bytes
        self.bank_bytes = bank_bytes
        self.rewrite_cross_bank = False
        self.want_softcall = False
        # Far-calls (abi.md 4.4.3): a cross-bank CALLSOFT is lowered to an
        # in-caller-bank 12-byte thunk that pushes a forward token (callee
        # offset, page, bank) and `jun __soft_ret`, which pops it, sets the
        # latch to the callee bank and dispatches. The latch change happens
        # only once control is in COMMON (__soft_ret), so no running instruction
        # is ever swapped out -- unlike the old common __tobank stubs. ON by
        # default; --no-far-calls restores the __tobank path for A/B testing.
        self.far_calls = True
        # Per-bank tail bytes reserved for far-call thunks/trampolines so they
        # fit in the caller's/callee's own bank instead of overflowing common.
        self.thunk_reserve = 0x180
        self.sec_bank: dict[int, int] = {}   # id(section) -> bank (-1 = common)
        self.placed: list[Placed] = []
        self.rom_image = bytearray()
        self.global_syms: dict[str, tuple[ObjFile, Symbol]] = {}
        self.linker_syms: dict[str, int] = {}
        self.warnings: list[str] = []

    @staticmethod
    def _is_common_fn(name: str) -> bool:
        """A ROM code section that belongs in the always-mapped common region:
        the reset vector, rodata, and every runtime/HAL/trampoline helper. The
        firmware's Rust functions (mangled `_R...`/`_Z...`) are banked."""
        # strip the leading .text./.reset/.rodata prefix to get the fn name
        for pfx in (".reset", ".rodata"):
            if name == pfx or name.startswith(pfx + "."):
                return True
        fn = name
        if name.startswith(".text."):
            fn = name[len(".text."):]
        elif name == ".text":
            return True  # a merged .text (no function-sections) stays common
        # runtime/HAL symbols: __foo, _start, reg_*, memset/memcpy/memmove
        return (fn.startswith("__") or fn.startswith("_start")
                or fn.startswith("reg_") or fn in ("memset", "memcpy", "memmove")
                or fn.startswith("__bankcall_"))

    @staticmethod
    def _is_leaf_helper(fn: str) -> bool:
        """A hand-written leaf runtime/HAL helper that holds no RAM frame and,
        by the abi.md 4.4.2 leaf rule (_assert_leaf_depth), makes no CALLSOFT to
        a compiled function. Used by _alloc_frames to drop such nodes from the
        frame call graph (they carry no frame liveness, and a widely-shared one
        like __memcpy_ram would otherwise fabricate spurious cycles). `fn` is a
        bare function name (no .text. prefix)."""
        return (fn.startswith("__") or fn.startswith("_start")
                or fn.startswith("reg_")
                or fn in ("memset", "memcpy", "memmove"))



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
        if self.banks:
            return self._place_rom_banked()
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

    # -- software-call return dispatch synthesis (abi.md 4.4.1/4.4.2) --------
    def _soft_ret_bytes(self, srtab_lo):
        """__soft_ret (verified encodings). Pops the 5-nibble return token off
        the P7 soft stack, top nibble first: BANK.hi, BANK.lo, page, off.hi,
        off.lo (SSP decremented as a full 8-bit pointer, SSP += 0xFF per pop).
        The 8-bit bank = (hi<<4)|lo is written to the ROM bank latch -- the LOW
        nibble to 4001 chip-3's output port (SRC 0x30, WRR) and the HIGH nibble
        to 4001 chip-4's output port (SRC 0x40, WRR) -- AND mirrored to the two
        __cur_bank RAM chars (lo @ 0x0F, hi @ 0x0E), restoring the CALLER's
        (up to 8-bit) bank before control returns. An 8-bit latch lifts the
        4-bit (16-bank) ceiling to 256 banks, which the >16-bank firmware needs.
        Then it builds the jin pointer in P4 (R8:R9) from __srtab (page offset
        `srtab_lo` = 0x80) + 2*page and JINs via the per-page stub. Scratch: R11
        (bank.lo), R10 (page/bank.hi), P4, P0; the return-value register D1 =
        P2:P3 (and P1) is NOT clobbered; P0 = R0:R1 carries the return OFFSET to
        the stub. jcn-nc -> .Lnocar = `jin p4`."""
        assert srtab_lo == 0x80, "synthesis pins __srtab at page offset 0x80"
        def dec_ssp():
            # SSP (R14:R15) += 0xFF  (== SSP - 1, 8-bit, carry R15->R14)
            return [0xf1, 0xdf,0x8f,0xbf, 0xdf,0x8e,0xbe]
        b = []
        # pop BANK.hi -> R10 ; latch hi <- R10 (SRC 0x40 WRR) ; cur_bank.hi 0x0E
        b += dec_ssp(); b += [0x2f,0xe9,0xba]          # src p7; rdm; xch r10 (bank.hi)
        b += [0x20,0x40,0x21,0xaa,0xe2]                # fim p0,0x40; src p0; ld r10; wrr
        b += [0x20,0x0e,0x21,0xaa,0xe0]                # fim p0,0x0e; src p0; ld r10; wrm
        # pop BANK.lo -> R11 ; latch lo <- R11 (SRC 0x30 WRR) ; cur_bank.lo 0x0F
        b += dec_ssp(); b += [0x2f,0xe9,0xbb]          # src p7; rdm; xch r11 (bank.lo)
        b += [0x20,0x30,0x21,0xab,0xe2]                # fim p0,0x30; src p0; ld r11; wrr
        b += [0x20,0x0f,0x21,0xab,0xe0]                # fim p0,0x0f; src p0; ld r11; wrm
        b += dec_ssp(); b += [0x2f,0xe9,0xba]          # src p7; rdm; xch r10 (page)
        b += dec_ssp(); b += [0x2f,0xe9,0xb0]          # src p7; rdm; xch r0  (off hi)
        b += dec_ssp(); b += [0x2f,0xe9,0xb1]          # src p7; rdm; xch r1  (off lo)
        b += [0xaa,0xf1,0xf5,0xb9]                     # ld r10; clc; ral; xch r9
        b += [0xd8,0xb8]                               # ldm 8; xch r8
        b += [0xa8]                                    # ld r8
        jcn_operand_idx = len(b) + 1
        b += [0x1a,0x00]                               # jcn nc, .Lnocar
        b += [0xf2,0xb8]                               # iac; xch r8
        nocar_idx = len(b)
        b += [0x39]                                    # jin p4   <- .Lnocar
        return b, nocar_idx, jcn_operand_idx

    # RAM scratch chars (chip0/reg0) used to carry P4:P5 (D2, the 2nd i16/ptr
    # argument pair) across a far-call forward dispatch, because __soft_ret
    # clobbers P4:P5 (R8..R11) while it computes the srtab jin pointer. The far
    # thunk saves R8..R11 here; a per-callee restore trampoline in the callee's
    # bank reloads them before entering the callee. Chars 0x0A..0x0D of reg0 are
    # free (BCD digits use chars 0..9; 0x0F is __cur_bank).
    FWD_SAVE = 0x0A   # R8 -> 0x0A, R9 -> 0x0B, R10 -> 0x0C, R11 -> 0x0D

    def _push_tok_bytes(self):
        """Leaf push helpers (JMS/BBL, hardware depth 1), all using ONLY the
        scratch pair P0 and the SSP (P7) -- no argument register (P1/D1=P2:P3,
        D2=P4:P5) is touched, so an i16 argument in D1/D2 survives. With the
        8-bit bank latch the token is 5 nibbles [off.lo, off.hi, page, bank.lo,
        bank.hi], low-address-first:
          __push_off: write R1(off lo) then R0(off hi) at SSP, SSP += 2
          __push_pg : write R0(page), then the CALLER bank read from the two
                      __cur_bank mirrors (lo @ 0x0F, hi @ 0x0E), SSP += 3.
                      Used for the RETURN token.
          __push_pb : write R0(page) then R1(bank.lo) at SSP, SSP += 2. Used by
                      the far thunk for the FORWARD token's page+bank.lo; the
                      thunk then pushes bank.hi with __push1 (the callee's bank
                      is an explicit value the packer fills, not __cur_bank).
          __push1   : write R0 at SSP, SSP += 1 (the forward token's bank.hi).
        SSP advances as a full 8-bit pointer (carry R15->R14). Returns
        (off_bytes, pg_bytes, pb_bytes, p1_bytes)."""
        def inc_ssp():
            return [0xf1, 0xd1,0x8f,0xbf, 0xd0,0x8e,0xbe]
        off = []
        off += [0x2f,0xa1,0xe0]; off += inc_ssp()   # src p7; ld r1; wrm; SSP++
        off += [0x2f,0xa0,0xe0]; off += inc_ssp()   # src p7; ld r0; wrm; SSP++
        off += [0xc0]                               # bbl 0
        pg = []
        pg += [0x2f,0xa0,0xe0]; pg += inc_ssp()     # src p7; ld r0(page); wrm; SSP++
        pg += [0x20,0x0f,0x21,0xe9,0xb0]            # fim p0,0x0f; src p0; rdm; xch r0 (cur bank.lo)
        pg += [0x2f,0xa0,0xe0]; pg += inc_ssp()     # src p7; ld r0(bank.lo); wrm; SSP++
        pg += [0x20,0x0e,0x21,0xe9,0xb0]            # fim p0,0x0e; src p0; rdm; xch r0 (cur bank.hi)
        pg += [0x2f,0xa0,0xe0]; pg += inc_ssp()     # src p7; ld r0(bank.hi); wrm; SSP++
        pg += [0xc0]                                # bbl 0
        # __push_pb: push R0(page) then R1(bank.lo) at SSP; SSP += 2. Takes an
        # EXPLICIT bank.lo already staged in R1 (the far-call forward token for a
        # cross-bank callee carries the CALLEE's bank). The thunk pushes bank.hi
        # separately with __push1. Leaf; touches only P0/SSP so args survive.
        pb = []
        pb += [0x2f,0xa0,0xe0]; pb += inc_ssp()     # src p7; ld r0(page); wrm; SSP++
        pb += [0x2f,0xa1,0xe0]; pb += inc_ssp()     # src p7; ld r1(bank.lo); wrm; SSP++
        pb += [0xc0]                                # bbl 0
        # __push1: push R0 (the forward token's bank.hi) at SSP; SSP += 1.
        p1 = []
        p1 += [0x2f,0xa0,0xe0]; p1 += inc_ssp()     # src p7; ld r0; wrm; SSP++
        p1 += [0xc0]                                # bbl 0
        # __save_p4p5: save P4:P5 (R8..R11) to the FWD_SAVE RAM chars. A COMMON
        # leaf (JMS/BBL, hardware depth 1) so the far thunk does not carry the
        # 20-byte inline save -- halving the thunk (40->~20 B) so it fits a
        # nearly-full bank's tail and rarely spills to the small common region.
        # Clobbers only P0 + ACC; args in P1-P3 and the pair being saved survive.
        S = self.FWD_SAVE
        sv = []
        sv += [0x20, S,   0x21, 0xa8, 0xe0]   # fim p0,S  ; src p0; ld r8 ; wrm
        sv += [0x20, S+1, 0x21, 0xa9, 0xe0]   # fim p0,S+1; src p0; ld r9 ; wrm
        sv += [0x20, S+2, 0x21, 0xaa, 0xe0]   # fim p0,S+2; src p0; ld r10; wrm
        sv += [0x20, S+3, 0x21, 0xab, 0xe0]   # fim p0,S+3; src p0; ld r11; wrm
        sv += [0xc0]                          # bbl 0
        return off, pg, pb, p1, sv

    def _synth_softcall(self):
        """Flat-image synthesis: reserve the per-page `jin p0` stub at STUB_OFF
        on every page in use, place __soft_ret + __srtab on a dedicated page,
        and register the linker-defined symbols __soft_ret / __srtab / __push_ret
        is NOT synthesized (inline push). Must run AFTER _place_rom (so rom_used
        is known) and BEFORE _apply (so the symbols resolve)."""
        if not getattr(self, "want_softcall", False):
            return
        if self.banks:
            return self._synth_softcall_banked()
        # Dedicated page for __soft_ret (@0x00) + push helpers + __srtab (@0x80),
        # just past the placed code.
        sr_page = (self.rom_used + PAGE - 1) // PAGE
        sr_base = sr_page * PAGE
        srtab_base = sr_base + 0x80
        self.soft_ret_base = sr_base
        # Dispatch-page layout (one 256-byte common page). __soft_ret grew to
        # ~82 B and __push_pg to ~41 B for the 8-bit bank latch (two latch
        # ports, 5-nibble token), so the old 0x40/0x60/0xa0 slots no longer fit.
        # __srtab is PINNED at 0x80 (soft_ret computes jin = __srtab + 2*page
        # assuming 0x80). New slots avoid [0x80,0xa0) and the 0xFC jin-p0 stub:
        #   soft_ret 0x00..0x51, push_off 0x54, push1 0x6a, srtab 0x80,
        #   push_pg 0xa0..0xc8, push_pb 0xd0..0xe4.
        self.push_off_base = sr_base + 0x54
        self.push1_base = sr_base + 0x6a
        self.push_pg_base = sr_base + 0xa0
        self.push_pb_base = sr_base + 0xd0
        self.save_p4p5_base = sr_base + 0xe6
        self.srtab_base = srtab_base
        self.soft_ret_page = sr_page
        self.linker_syms[SOFT_RET_SYM] = sr_base
        self.linker_syms[PUSH_OFF_SYM] = self.push_off_base
        self.linker_syms[PUSH_PG_SYM] = self.push_pg_base
        self.linker_syms[PUSH_PB_SYM] = self.push_pb_base
        self.linker_syms[PUSH1_SYM] = self.push1_base
        self.linker_syms[SAVE_P4P5_SYM] = self.save_p4p5_base
        self.linker_syms[SRTAB_SYM] = srtab_base
        # the synthesized page occupies the whole 256 bytes (stub at 0xFC too)
        self.rom_used = max(self.rom_used, sr_base + PAGE)
        # number of pages now in the image (for per-page stubs)
        self.n_pages = (self.rom_used + PAGE - 1) // PAGE
        if self.rom_used > self.rom_len:
            raise LinkError(
                f"soft-call synthesis overflows ROM budget "
                f"({self.rom_used} > {self.rom_len}); raise --max")

    def _synth_softcall_banked(self):
        """Banked soft-call: the return-dispatch region (__soft_ret @ 0x00, the
        push helpers, __srtab @ 0x80) lives on ONE common page (mapped in every
        bank); the per-page `jin p0` stub sits at STUB_OFF on every page. The
        cross-bank `__tobank_<addr>` set-latch stubs are packed into the common
        FREE bytes (the gap before the dispatch page, the unused parts of the
        dispatch page, and the tail after it) so the common region stays small.
        Runs after _place_rom (banked) / before _build_banked_images."""
        self._common_used_fns = self.common_used  # functions end here
        sr_page = (self.common_used + PAGE - 1) // PAGE
        sr_base = sr_page * PAGE
        if sr_base + PAGE > self.common_bytes:
            raise LinkError(
                f"banked soft-call: no room for the return-dispatch page in the "
                f"common region (need page at 0x{sr_base:x}, common_bytes "
                f"0x{self.common_bytes:x}); raise --common-bytes")
        srtab_base = sr_base + 0x80
        self.soft_ret_base = sr_base
        # Dispatch-page layout (one 256-byte common page). __soft_ret grew to
        # ~82 B and __push_pg to ~41 B for the 8-bit bank latch (two latch
        # ports, 5-nibble token), so the old 0x40/0x60/0xa0 slots no longer fit.
        # __srtab is PINNED at 0x80 (soft_ret computes jin = __srtab + 2*page
        # assuming 0x80). New slots avoid [0x80,0xa0) and the 0xFC jin-p0 stub:
        #   soft_ret 0x00..0x51, push_off 0x54, push1 0x6a, srtab 0x80,
        #   push_pg 0xa0..0xc8, push_pb 0xd0..0xe4.
        self.push_off_base = sr_base + 0x54
        self.push1_base = sr_base + 0x6a
        self.push_pg_base = sr_base + 0xa0
        self.push_pb_base = sr_base + 0xd0
        self.save_p4p5_base = sr_base + 0xe6
        self.srtab_base = srtab_base
        self.soft_ret_page = sr_page
        self.linker_syms[SOFT_RET_SYM] = sr_base
        self.linker_syms[PUSH_OFF_SYM] = self.push_off_base
        self.linker_syms[PUSH_PG_SYM] = self.push_pg_base
        self.linker_syms[PUSH_PB_SYM] = self.push_pb_base
        self.linker_syms[PUSH1_SYM] = self.push1_base
        self.linker_syms[SAVE_P4P5_SYM] = self.save_p4p5_base
        self.linker_syms[SRTAB_SYM] = srtab_base
        self.common_used = max(self.common_used, sr_base + PAGE)
        # pages per bank image = common_bytes + bank_bytes, all 256-byte pages.
        self.n_pages = (self.common_bytes + self.bank_bytes) // PAGE
        if getattr(self, "far_calls", True):
            self._synth_far_thunks()
        else:
            self._synth_bank_stubs()

    def _synth_far_thunks(self):
        """Far-call lowering (abi.md 4.4.3). For every distinct cross-bank
        CALLSOFT (callee_addr, caller_bank) pair, allocate a 12-byte forward-
        token thunk IN THE CALLER'S BANK (its banked tail). The thunk pushes the
        forward token (callee offset, page, callee_bank) with the common push
        helpers and `jun __soft_ret`; the cross-bank `jun callee` site is
        rewritten to `jun thunk` (same 2 bytes -- NO section growth). The thunk
        is correct in the banked region precisely because it NEVER touches the
        bank latch: every one of its instructions runs under the caller's own
        (still-mapped) bank, and the single latch switch happens only after
        control reaches COMMON __soft_ret. This removes the per-callee common
        __tobank stubs entirely; common grows only by the one __push_pb helper.

        Keyed by (callee_addr, callee_bank, caller_bank): a given callee
        reached from two different caller banks needs a thunk in each (a banked
        PC exists in every bank, and the thunk must sit in the bank the call
        site is in). The callee_bank MUST be part of the key because a banked
        PC (e.g. 0x800) is identical in every bank, so two DIFFERENT callees in
        different banks can share the same `callee_addr`; keying by
        (addr, host_bank) alone made the second such call site overwrite the
        first in far_thunks, so both cross-bank calls from one caller bank
        dispatched to the SAME callee (the deep-i8-chain N-2/doubling
        regression -- two cross-bank calls in one function went to one callee).
        """
        self.bank_stubs = {}        # unused in far-call mode (legacy name kept)
        self.far_thunks = {}        # (callee_addr, callee_bank, host_bank) -> (abs, callee_addr, callee_bank, is_common)
        self.far_thunks_common = {} # (callee_addr, callee_bank) -> (abs, callee_addr, callee_bank)
        # which cross-bank call sites exist, and from where?
        banked_pairs = set()        # (callee_addr, callee_bank, caller_bank)
        common_pairs = set()        # (callee_addr, callee_bank)
        for _loc, frm, to, addr in self._cross_bank_calls():
            if addr is None or to == -1:
                continue
            if frm == -1:
                # COMMON caller (e.g. crt0's `jun main`). Its thunk MUST live in
                # common: when the call fires the latch may still point at a
                # different bank, so a thunk in the callee's bank would not be
                # mapped. A common thunk is always mapped and never touches the
                # latch itself (the switch happens in __soft_ret).
                common_pairs.add((addr, to))
            else:
                banked_pairs.add((addr, to, frm))
        THUNK = 24   # jms __save_p4p5 (2) + 18 B push/dispatch (8-bit bank) + slack
        self._common_thunk_slots = None   # lazily opened _common_free_slots gen
        # banked-caller thunks: prefer the caller's own bank tail; fall back to
        # a common slot (always mapped, equally correct) when the bank is full.
        for (addr, callee_bank, host_bank) in sorted(banked_pairs):
            base = self._alloc_in_bank(host_bank, THUNK)
            if base is None:
                base = self._alloc_common_thunk(THUNK)
                self.far_thunks_common[(addr, callee_bank)] = (base, addr, callee_bank)
                self.linker_syms[f"__farc_{addr:04x}_b{callee_bank:x}"] = base
                # record the host-bank site routes to the common thunk
                self.far_thunks[(addr, callee_bank, host_bank)] = (base, addr, callee_bank, True)
            else:
                self.far_thunks[(addr, callee_bank, host_bank)] = (base, addr, callee_bank, False)
            self.linker_syms[f"__far_{addr:04x}_c{callee_bank:x}_b{host_bank:x}"] = base
        # common-caller thunks: in common free space (mapped in every bank).
        for (addr, callee_bank) in sorted(common_pairs):
            base = self._alloc_common_thunk(THUNK)
            self.far_thunks_common[(addr, callee_bank)] = (base, addr, callee_bank)
            self.linker_syms[f"__farc_{addr:04x}_b{callee_bank:x}"] = base
        # Restore trampolines: ONE per distinct cross-bank callee, in the
        # CALLEE's bank. A far thunk saves P4:P5 (R8..R11) to RAM then routes the
        # forward token to the callee's trampoline (not the callee directly);
        # __soft_ret dispatches there with latch=callee_bank (so the callee bank,
        # holding the trampoline, is mapped), and the trampoline reloads R8..R11
        # and `jun`s the real callee. This makes the 2nd i16/pointer argument
        # pair (D2 = P4:P5), which __soft_ret clobbers, survive the cross-bank
        # call. Keyed by (callee_addr, callee_bank).
        REST = 14   # 4x(fim/src/rdm/xch)=4*? -> see _restore_tramp_bytes; <= 14
        self.far_restore = {}       # (callee_addr, callee_bank) -> (abs, callee_addr, callee_bank, is_common)
        callees = set()
        for tup in self.far_thunks.values():
            callees.add((tup[1], tup[2]))          # (callee_addr, callee_bank)
        for v in self.far_thunks_common.values():
            callees.add((v[1], v[2]))
        for (callee_addr, callee_bank) in sorted(callees):
            tb = self._restore_tramp_bytes(callee_addr)
            base = self._alloc_in_bank(callee_bank, len(tb))
            is_common = False
            if base is None:
                # callee bank full: a COMMON trampoline works too (common is
                # mapped in every bank, including the callee's; its `jun callee`
                # reaches the now-mapped callee bank).
                base = self._alloc_common_thunk(len(tb))
                is_common = True
            self.far_restore[(callee_addr, callee_bank)] = (base, callee_addr, callee_bank, is_common)
            self.linker_syms[f"__frest_{callee_addr:04x}_b{callee_bank:x}"] = base
        # recompute rom_used (unchanged: banks are uniform width)
        self.rom_used = self.common_bytes + self.bank_bytes

    def _restore_tramp_bytes(self, callee_addr):
        """Callee-bank restore trampoline: reload P4:P5 (R8..R11) from the RAM
        save chars, then `jun callee`. Uses P0 (R0:R1) as the RAM address pair
        and writes into R8..R11 directly -- no temp-register conflict, and no
        argument pair other than the P4:P5 it is restoring is touched."""
        S = self.FWD_SAVE
        b = []
        b += [0x20, S,   0x21, 0xe9, 0xb8]   # fim p0,S  ; src p0; rdm; xch r8
        b += [0x20, S+1, 0x21, 0xe9, 0xb9]   # fim p0,S+1; src p0; rdm; xch r9
        b += [0x20, S+2, 0x21, 0xe9, 0xba]   # fim p0,S+2; src p0; rdm; xch r10
        b += [0x20, S+3, 0x21, 0xe9, 0xbb]   # fim p0,S+3; src p0; rdm; xch r11
        b += [0x40 | ((callee_addr >> 8) & 0xF), callee_addr & 0xFF]  # jun callee
        return b

    def _alloc_common_thunk(self, width):
        """Reserve `width` bytes in common free space for a far thunk (mapped in
        every bank). Opens the free-slot generator lazily."""
        if self._common_thunk_slots is None:
            self._common_thunk_slots = self._common_free_slots(width)
        try:
            base = next(self._common_thunk_slots)
        except StopIteration:
            raise LinkError(
                f"banked soft-call (far-calls): no room for a {width}-byte far "
                f"thunk in common free space (common_bytes "
                f"0x{self.common_bytes:x}); raise --common-bytes/--bank-bytes")
        self.common_used = max(self.common_used, base + width)
        return base

    def _alloc_in_bank(self, bank, width):
        """Reserve `width` bytes in `bank`'s banked region, after the placed
        functions. A thunk/trampoline contains only page-independent ops
        (FIM/SRC/LD/WRM/JMS/JUN), so it MAY straddle a 256-byte page; it must
        only avoid overwriting the per-page `jin p0` stub byte at STUB_OFF.
        Advances self.bank_fill[bank]; returns None on overflow (caller falls
        back to a common slot)."""
        pos = self.common_bytes + self.bank_fill[bank]
        limit = self.common_bytes + self.bank_bytes
        while True:
            # if the span would cover a STUB_OFF byte on any page it crosses,
            # bump past the nearest such byte.
            span_lo, span_hi = pos, pos + width
            bumped = False
            p = pos & ~(PAGE - 1)
            while p < span_hi:
                stub = p + STUB_OFF
                if span_lo <= stub < span_hi:
                    pos = stub + 1
                    bumped = True
                    break
                p += PAGE
            if bumped:
                continue
            if pos + width > limit:
                return None   # bank tail full; caller falls back to common
            self.bank_fill[bank] = (pos + width) - self.common_bytes
            return pos

    def _write_far_thunks(self, img, bank):
        """Emit the far-call thunk bytes for `bank` into its image. The thunk:
          - saves P4:P5 (R8..R11) to the FWD_SAVE RAM chars (so the 2nd i16/ptr
            arg survives __soft_ret clobbering P4:P5 during dispatch);
          - pushes a forward token (tramp_off, tramp_page, callee_bank) for the
            callee's RESTORE TRAMPOLINE (in the callee bank);
          - `jun __soft_ret`, which pops the token, sets latch=callee_bank and
            dispatches to the trampoline, which reloads P4:P5 and `jun`s callee.
        Uses only P0 + ACC (+ the SSP via the push helpers). The latch changes
        only inside common __soft_ret, so no running instruction is swapped out.
        All addresses are final (packer placement done)."""
        push_off = self.push_off_base
        push_pb = self.push_pb_base
        push1 = self.push1_base
        soft_ret = self.soft_ret_base
        S = self.FWD_SAVE
        def thunk_bytes(callee_addr, callee_bank):
            tramp = self.far_restore[(callee_addr, callee_bank)][0]
            off = tramp & 0xFF                 # trampoline page-local offset
            page = (tramp >> 8) & 0x0F         # trampoline page nibble
            bank_lo = callee_bank & 0x0F
            bank_hi = (callee_bank >> 4) & 0x0F
            save = self.save_p4p5_base
            b = [
                0x50 | ((save >> 8) & 0xF), save & 0xFF,          # jms __save_p4p5
                0x20, off,                              # fim p0, off   (R0=off.hi,R1=off.lo)
                0x50 | ((push_off >> 8) & 0xF), push_off & 0xFF,  # jms __push_off
                0x20, (page << 4) & 0xF0,               # fim p0, page<<4 (R0.hi nibble=page)
                0xD0 | bank_lo,                         # ldm callee_bank.lo
                0xB1,                                   # xch r1  (R1 = bank.lo)
                0x50 | ((push_pb >> 8) & 0xF), push_pb & 0xFF,    # jms __push_pb (page, bank.lo)
                0xD0 | bank_hi,                         # ldm callee_bank.hi
                0xB0,                                   # xch r0  (R0 = bank.hi)
                0x50 | ((push1 >> 8) & 0xF), push1 & 0xFF,        # jms __push1 (bank.hi)
                0x40 | ((soft_ret >> 8) & 0xF), soft_ret & 0xFF,  # jun __soft_ret
            ]
            return b
        # banked-caller thunks: only in their host bank. Entries that fell back
        # to common (is_common=True) are emitted by the common loop below.
        for (addr, ckbank, host_bank), tup in getattr(self, "far_thunks", {}).items():
            base, callee_addr, callee_bank, is_common = tup
            if is_common or host_bank != bank:
                continue
            for i, by in enumerate(thunk_bytes(callee_addr, callee_bank)):
                img[base + i] = by
        # common-caller thunks: live in common, so emit into EVERY bank image.
        for (addr, callee_bank), (base, callee_addr, cb) in \
                getattr(self, "far_thunks_common", {}).items():
            for i, by in enumerate(thunk_bytes(callee_addr, cb)):
                img[base + i] = by
        # restore trampolines: in-bank ones only in their callee bank; common
        # ones into every bank image.
        for (callee_addr, callee_bank), (base, ca, cb, is_common) in \
                getattr(self, "far_restore", {}).items():
            if not is_common and callee_bank != bank:
                continue
            for i, by in enumerate(self._restore_tramp_bytes(callee_addr)):
                img[base + i] = by

    def _common_free_slots(self, width):
        """Yield `width`-byte free slots in the common region for synthesized
        stubs, avoiding: the placed common functions [0, _common_used_fns), the
        dispatch block (soft_ret 0..0x40, push_off 0x40..0x58, push_pg
        0x60..0x7a, srtab 0x80..0xa0 on the dispatch page), and the per-page
        `jin p0` stub byte at STUB_OFF on every page. A slot never straddles a
        256-byte page (so a 12-byte stub's `jun` stays simple)."""
        srp = self.soft_ret_page
        # dispatch page layout (8-bit bank latch): soft_ret 0x00..0x52,
        # push_off 0x54..0x69, push1 0x6a..0x75, srtab 0x80..0xa0,
        # push_pg 0xa0..0xc9, push_pb 0xd0..0xe5. Reserve generously.
        used_on_srp = set()
        for a, b in [(0x00, 0x54), (0x54, 0x6a), (0x6a, 0x76),
                     (0x80, 0xa0), (0xa0, 0xca), (0xd0, 0xe6), (0xe6, 0xfc)]:
            used_on_srp.update(range(a, b))
        start = self._common_used_fns
        for page_base in range(0, self.common_bytes, PAGE):
            if page_base < (start & ~(PAGE - 1)):
                continue
            off = 0
            while off + width <= PAGE:
                abs_lo = page_base + off
                if abs_lo < start:
                    off += 1; continue
                win = range(off, off + width)
                if STUB_OFF in win:
                    off = STUB_OFF + 1; continue
                if page_base == srp * PAGE and any(o in used_on_srp for o in win):
                    off += 1; continue
                if page_base + off + width > self.common_bytes:
                    break
                yield page_base + off
                off += width
            # advance to next page handled by outer loop

    def _synth_bank_stubs(self):
        """Register a common `__tobank` set-latch stub (10 B) for every distinct
        cross-bank CALLSOFT (target_addr, callee_bank) pair, packed into common
        free slots. The stub sets the ROM bank latch + __cur_bank mirror to the
        callee's bank, then `jun`s the callee. Cross-bank `jun callee` sites are
        rewritten to the matching stub in _build_banked_images; the token's 4th
        (bank) nibble restores the caller's bank in __soft_ret. Keyed by
        (addr, bank) because a banked PC exists in every bank."""
        pairs = self.cross_bank_callee_pairs()  # {(addr, bank) -> bank}
        self.bank_stubs = {}                     # (addr, bank) -> stub_base
        slots = self._common_free_slots(12)
        for (addr, bank) in sorted(pairs):
            try:
                base = next(slots)
            except StopIteration:
                raise LinkError(
                    f"banked soft-call: no room for {len(pairs)} __tobank "
                    f"stubs ({len(pairs)*12} B) in common free space "
                    f"(common_bytes 0x{self.common_bytes:x}); raise "
                    f"--common-bytes or reduce cross-bank calls")
            self.bank_stubs[(addr, bank)] = base
            self.linker_syms[f"__tobank_{addr:04x}_b{bank:x}"] = base
            self.common_used = max(self.common_used, base + 12)

    def _write_bank_stubs(self, img):
        """Emit the __tobank stub bytes into a bank image (common region,
        identical in every bank). 12 bytes: set the ROM bank latch (chip 3) and
        the __cur_bank RAM mirror (0x0F) to the callee bank, then jun it. The
        bank is re-loaded (ldm B) for the __cur_bank write rather than assuming
        WRR preserves ACC."""
        for (addr, bank), sb in getattr(self, "bank_stubs", {}).items():
            B = bank & 0xF
            body = [0x20,0x30, 0x21, 0xd0 | B, 0xe2,   # fim p0,0x30; src p0; ldm B; wrr
                    0x20,0x0f, 0x21, 0xd0 | B, 0xe0,   # fim p0,0x0f; src p0; ldm B; wrm
                    0x40 | ((addr >> 8) & 0xF), addr & 0xFF]  # jun <addr>  (12 bytes)
            for i, by in enumerate(body):
                img[sb + i] = by

    def _emit_softcall(self):
        """Write the synthesized soft-call bytes into self.rom_image (FLAT path).
        Called from _apply after relocations are patched. The banked path calls
        _write_softcall on each per-bank image instead."""
        if not getattr(self, "want_softcall", False) or self.banks:
            return
        self._write_softcall(self.rom_image, self.n_pages)

    def _write_softcall(self, img, n_pages):
        """Write the per-page `jin p0` stubs, __soft_ret, the push helpers and
        __srtab into image `img` (which spans `n_pages` 256-byte pages). Shared
        by the flat and banked emitters; all addresses are self.* bases set by
        _synth_softcall[_banked] (common PCs, identical in every bank)."""
        need = n_pages * PAGE
        if len(img) < need:
            img.extend(b"\x00" * (need - len(img)))
        # per-page jin p0 stub at STUB_OFF on every page
        for p in range(n_pages):
            img[p * PAGE + STUB_OFF] = JIN_P0
        # __soft_ret body
        body, nocar_idx, jcn_operand_idx = self._soft_ret_bytes(self.srtab_base & 0xFF)
        sr = self.soft_ret_base
        for i, byte in enumerate(body):
            img[sr + i] = byte
        # patch the jcn-nc target (.Lnocar) low byte (page-local)
        nocar_addr = sr + nocar_idx
        img[sr + jcn_operand_idx] = nocar_addr & 0xFF
        # __push_off / __push_pg / __push_pb / __push1 leaf helpers (8-bit SSP
        # advance; no relocations). __push_pb + __push1 push the far-call
        # forward-token page + bank.lo + bank.hi (8-bit bank latch).
        off_b, pg_b, pb_b, p1_b, sv_b = self._push_tok_bytes()
        for i, byte in enumerate(off_b):
            img[self.push_off_base + i] = byte
        for i, byte in enumerate(pg_b):
            img[self.push_pg_base + i] = byte
        for i, byte in enumerate(pb_b):
            img[self.push_pb_base + i] = byte
        for i, byte in enumerate(p1_b):
            img[self.push1_base + i] = byte
        for i, byte in enumerate(sv_b):
            img[self.save_p4p5_base + i] = byte
        # __srtab: 16 entries `jun <stub on page k>` = 0x4<k>, 0xFC
        st = self.srtab_base
        for k in range(16):
            img[st + 2 * k] = 0x40 | (k & 0x0F)
            img[st + 2 * k + 1] = STUB_OFF

    def _assert_leaf_depth(self):
        """abi.md 4.4.2 item 3: every function reached by a JMS (a hardware-stack
        call) must itself make no JMS to a non-leaf -- i.e. the JMS-reachable
        subgraph is <= 3 deep. The compiled firmware uses JMS only for calls to
        the hand-written leaf rt/HAL helpers (__*, memcpy/memset/memmove), which
        by construction contain no JMS to a compiled function. We verify by
        scanning every placed ROM section for a JMS whose target resolves to a
        section that itself contains a JMS (depth > 2 via hardware stack)."""
        if not getattr(self, "want_softcall", False):
            return
        # Build: for each placed section, does it contain a JMS? (opcode 0x5X)
        # and collect JMS call edges target-addr -> callee section.
        # A conservative structural check: a JMS target that is itself a section
        # containing a JMS, that in turn contains a JMS, exceeds depth 3.
        sec_has_jms = {}
        for p in self.placed:
            if p.kind != "rom":
                continue
            data = p.sec.data
            has = False
            i = 0
            # scan 2-byte-aware: JMS is 0x5X 0xYY; also skip other 2-byte ops
            while i < len(data):
                op = data[i] & 0xF0
                two = op in (0x10, 0x20 if (data[i] & 0x01) == 0 else 0xFF,
                             0x40, 0x50, 0x70)
                if (data[i] & 0xF0) == 0x50:
                    has = True
                # advance: FIM/JCN/JUN/JMS/ISZ are 2 bytes; FIM is 0x2X even
                if (data[i] & 0xF0) in (0x10, 0x40, 0x50, 0x70) or \
                   ((data[i] & 0xF1) == 0x20):
                    i += 2
                else:
                    i += 1
            sec_has_jms[id(p.sec)] = has
        # (Structural depth proof is bounded by the leaf-helper invariant the
        # backend enforces: compiled code emits JMS ONLY to __*/mem* leaf
        # helpers, never to a compiled function. We assert that invariant held
        # by checking no JMS edge targets a firmware (non-common) section.)
        for obj in self.objs:
            for sec in obj.sections:
                rels = obj.relocs.get(sec.index)
                if not rels or sec.base < 0:
                    continue
                for rel in rels:
                    if rel.rtype != R_I4004_ADDR12:
                        continue
                    # is this reloc on a JMS? opcode byte at rel.offset
                    if classify(sec.name) != "rom":
                        continue
                    opcode = sec.data[rel.offset] & 0xF0 if rel.offset < len(sec.data) else 0
                    if opcode != 0x50:
                        continue
                    sym = obj.symbols[rel.sym]
                    nm = sym.name
                    if nm and not (nm.startswith("__") or nm in
                                   ("memset", "memcpy", "memmove")):
                        raise LinkError(
                            f"{self._loc(obj, sec, rel)}: JMS to non-leaf "
                            f"'{nm}' would overflow the 3-level hardware stack "
                            f"(abi.md 4.4.2); compiled calls must be CALLSOFT")


    def _page_align_section(self, obj, sec, base):
        """Insert NOP padding so no PAGE8/FIN8-relocated 2-byte instruction in
        `sec` straddles a 256-byte page or targets a different page than its
        operand byte. After far-branch relaxation a conditional branch is
        `JCN !cc,.Lcont ; JUN far`; the near JCN still uses an 8-bit page-local
        field, so if the JCN sits in the last few bytes of a page its +N target
        rolls onto the next page. We pad (0x00 == NOP) BEFORE such an
        instruction to push the whole thing onto the next page.

        Operates on `sec.data`/its relocs in place, shifting reloc offsets for
        every reloc at/after an insertion. Only same-section (local) PAGE8/FIN8
        targets are checked here; cross-section ones are caught at resolve time.
        Returns True if it changed anything (so the caller re-runs placement).

        `base` is sec's assigned ROM address. PAGE8/FIN8 reloc.offset points at
        the OPERAND byte; the opcode is at offset-1.
        """
        rels = obj.relocs.get(sec.index)
        if not rels:
            return False
        # Build S (target) for a reloc if its symbol is local to THIS section.
        def local_target(rel):
            sym = obj.symbols[rel.sym]
            STT_SECTION = 3
            if (sym.info & 0xf) == STT_SECTION or sym.name == "":
                if sym.shndx == sec.index:
                    return sec.base + rel.addend  # section-relative
                return None
            if sym.shndx == sec.index:
                return sec.base + sym.value + rel.addend
            return None
        changed = False
        # Process relocs in increasing offset so earlier inserts shift later ones.
        page_relocs = sorted((r for r in rels
                              if r.rtype in (R_I4004_PAGE8, R_I4004_FIN8)),
                             key=lambda r: r.offset)
        for rel in page_relocs:
            operand_addr = base + rel.offset
            operand_page = operand_addr // PAGE
            opcode_page = (operand_addr - 1) // PAGE
            tgt = local_target(rel)
            tgt_page = (tgt // PAGE) if tgt is not None else operand_page
            # ok iff opcode, operand and (local) target share one page
            if operand_page == opcode_page and operand_page == tgt_page:
                continue
            # pad the 2-byte instruction (opcode at rel.offset-1) to the next
            # page start: insert (PAGE - opcode_offset_in_page) NOPs before it.
            op_off = rel.offset - 1
            pad = PAGE - ((base + op_off) % PAGE)
            if pad == 0 or pad >= PAGE:
                continue
            ins_at = op_off
            sec.data = sec.data[:ins_at] + b"\x00" * pad + sec.data[ins_at:]
            sec.size += pad
            for r in rels:
                if r.offset >= ins_at:
                    r.offset += pad
            # local symbols defined after ins_at in this section shift too
            for s in obj.symbols:
                if s.shndx == sec.index and s.value >= ins_at:
                    s.value += pad
            changed = True
        return changed

    def _place_rom_banked(self):
        """xl bank-switched placement. Common ROM sections (.reset/.rodata +
        runtime/HAL/trampoline helpers) go in the common region [0,common_bytes)
        shared by every bank. Firmware functions are greedily bin-packed into
        banks' banked regions [common_bytes, common_bytes+bank_bytes). Each
        banked function's ADDRESS is common_bytes + its offset within its bank
        (so the same PC in every bank). sec_bank records the assignment; -1 =
        common. (Trampoline synthesis + cross-bank-call rewrite is a follow-up;
        this first lands correct classification, placement and the bank .map.)"""
        inputs = self._ordered_rom_inputs()
        common_items = [(o, s) for (o, s) in inputs if self._is_common_fn(s.name)]
        banked_items = [(o, s) for (o, s) in inputs if not self._is_common_fn(s.name)]

        # Common region: .reset first (at 0), then the rest. The common region
        # holds the HAND-WRITTEN runtime/HAL/trampoline asm, whose JCN/ISZ are
        # page-local and are NOT touched by I4004BranchRelax (a CodeGen pass).
        # So each common CODE section must stay whole inside one 256-byte page
        # (like --pad-pages). The banked firmware is CodeGen-relaxed and needs no
        # padding.
        addr = 0
        for obj, sec in common_items:
            align = max(sec.addralign, 1)
            addr = (addr + align - 1) & ~(align - 1)
            is_code = not (sec.name == ".rodata" or sec.name.startswith(".rodata"))
            if is_code and sec.size > 0:
                if sec.size > PAGE:
                    raise LinkError(
                        f"{obj.path}:{sec.name} is {sec.size} bytes > one 256-byte "
                        f"page; a common (runtime/trampoline) section cannot span a "
                        f"page (its JCN/ISZ are page-local). Split it.")
                if (addr // PAGE) != ((addr + sec.size - 1) // PAGE):
                    addr = (addr + PAGE - 1) & ~(PAGE - 1)  # bump to next page
            sec.base = addr
            self.sec_bank[id(sec)] = -1
            self.placed.append(Placed(obj, sec, addr, -1, "rom"))
            addr += sec.size
        self.common_used = addr
        if addr > self.common_bytes:
            raise LinkError(
                f"common ROM region overflow: {addr} bytes > {self.common_bytes} "
                f"(0x{self.common_bytes:x}); common holds .reset/crt0/HAL/rt + "
                f"trampolines -- shrink the hot-runtime set or enlarge the common region")

        # Banked region: greedy first-fit bin-pack by descending size. Then run
        # the page-align pass (NOP-pad page-local instructions off page ends) and
        # RE-PACK to a fixed point, since padding grows sections and moves bases.
        # Order functions by call-graph clustering (a callee right after its
        # caller) so first-fit keeps call chains within one bank -> far fewer
        # cross-bank calls (and thus __tobank stubs + runtime latch switches).
        pack_order = self._callgraph_order(banked_items)
        for _iter in range(8):
            base = self.common_bytes
            bank_fill = [0] * self.banks
            # drop prior banked placements (keep common ones)
            self.placed = [p for p in self.placed
                           if self.sec_bank.get(id(p.sec), -1) == -1]
            cur = 0  # pure next-fit in call-graph order: a DFS-adjacent call
                     # chain stays in one bank; advance only when it won't fit.
            # Keep a soft per-bank tail reserve for far-call thunks/trampolines
            # (they must live in the caller's / callee's own bank). A function
            # larger than bank_bytes-reserve is allowed to occupy an otherwise
            # empty bank at full size (its out-calls' thunks then fall back to
            # common). reserve=0 when far-calls are off.
            reserve = (self.thunk_reserve
                       if (getattr(self, "want_softcall", False)
                           and getattr(self, "far_calls", True))
                       else 0)
            def fits(cur, size):
                soft = self.bank_bytes - reserve
                if bank_fill[cur] + size <= soft:
                    return True
                # allow a lone oversized function to use the whole bank
                return bank_fill[cur] == 0 and size <= self.bank_bytes
            for obj, sec in pack_order:
                if sec.size > self.bank_bytes:
                    raise LinkError(
                        f"{obj.path}:{sec.name} is {sec.size} bytes > one bank's "
                        f"banked region {self.bank_bytes}; split the function")
                if not fits(cur, sec.size):
                    cur += 1
                    while cur < self.banks and not fits(cur, sec.size):
                        cur += 1
                    if cur >= self.banks:
                        raise LinkError(
                            f"no bank has room for {obj.path}:{sec.name} "
                            f"({sec.size} B) in next-fit order; raise --banks")
                sec.base = base + bank_fill[cur]
                self.sec_bank[id(sec)] = cur
                self.placed.append(Placed(obj, sec, sec.base, -1, "rom"))
                bank_fill[cur] += sec.size
            # page-align each banked section at its assigned base
            changed = False
            for obj, sec in banked_items:
                if self._page_align_section(obj, sec, sec.base):
                    changed = True
            if not changed:
                break
        self.bank_fill = bank_fill
        # rom_used = common + the largest bank (image size per bank is uniform).
        self.rom_used = self.common_bytes + self.bank_bytes

    def _callgraph_order(self, banked_items):
        """Order banked (obj, sec) items so a caller is immediately followed by
        the callees it calls (DFS over the ADDR12 call graph restricted to
        banked sections), falling back to descending size for anything not
        reached. First-fit bin-packing this order clusters each call chain into
        one bank, minimising cross-bank calls. Pure reordering -- placement and
        correctness are unchanged."""
        # Map a banked section id -> (obj, sec); and section defining a global.
        items = list(banked_items)
        by_id = {id(s): (o, s) for (o, s) in items}
        # name -> section id for call-target resolution.
        name_to_sec = {}
        for o, s in items:
            # the function symbol shares the section; find its defining global
            for sym in o.symbols:
                if sym.shndx < len(o.sections) and o.sections[sym.shndx] is s \
                        and (sym.info >> 4) == 1 and sym.name:
                    name_to_sec[sym.name] = id(s)
        # edges: section id -> list of callee section ids (banked only).
        edges = {id(s): [] for (o, s) in items}
        for o, s in items:
            for rel in o.relocs.get(s.index, []):
                if rel.rtype != R_I4004_ADDR12:
                    continue
                sym = o.symbols[rel.sym]
                tid = None
                if (sym.info & 0xf) == 3 or sym.name == "":   # section symbol
                    tsec = o.sections[sym.shndx]
                    if id(tsec) in by_id:
                        tid = id(tsec)
                elif sym.name in name_to_sec:
                    tid = name_to_sec[sym.name]
                if tid is not None and tid != id(s):
                    edges[id(s)].append(tid)
        # DFS from the largest functions first (entry points tend to be large),
        # then anything unvisited, to produce a caller-then-callees order.
        order, seen = [], set()
        roots = [id(s) for (o, s) in sorted(items, key=lambda it: -it[1].size)]
        def dfs(nid):
            stack = [nid]
            while stack:
                n = stack.pop()
                if n in seen:
                    continue
                seen.add(n)
                order.append(by_id[n])
                # push callees (reverse so first-listed is visited first)
                for c in reversed(edges.get(n, [])):
                    if c not in seen:
                        stack.append(c)
        for r in roots:
            dfs(r)
        # any items not reached (shouldn't happen) appended by size
        for o, s in sorted(items, key=lambda it: -it[1].size):
            if id(s) not in seen:
                order.append((o, s)); seen.add(id(s))
        return order


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

    def _alloc_frames(self):
        """Per-function RAM frame allocation via a call-graph OVERLAY (abi.md
        4.4 / brief decision 1). The backend addresses every spill/reload and
        every frame-local pointer relative to an undefined symbol __frame_<fn>
        (char offset within fn's frame). Here we:

          1. discover each fn's frame SIZE = max(addend)+2 over the relocs that
             reference its __frame_<fn> (chars; a SPILL16 walks up to 4 chars so
             the +2 term is conservative per slot and the max already covers the
             widest slot touched);
          2. build the call graph (caller fn -> callee fn) from ADDR12 relocs
             between distinct firmware .text sections -- an OVER-approximation
             (it also counts a tail `jun`), which is SAFE: it can only force two
             frames apart that might not strictly need to be;
          3. topologically order and assign base(fn) = max over callers P of
             (base(P)+size(P)); a function's frame is live across all its
             transitive callees, so this guarantees no two simultaneously-live
             frames overlap while letting siblings reuse the same chars;
          4. FAIL LOUDLY on a cycle (recursion would need an SSP-relative frame,
             which this scheme does not provide);
          5. place all frames in CM-RAM bank 0, after the firmware data+bss and
             clear of the soft stack, and assert the total fits.

        Sets linker_syms['__frame_<fn>'] = absolute char base. Functions that
        never touch a frame get no symbol (none is referenced)."""
        FRAME_PREFIX = "__frame_"
        # 1. frame sizes, and map fn-name -> its .text section id (for the graph)
        frame_size = {}          # fn-name -> size in chars
        for obj in self.objs:
            for sec in obj.sections:
                rels = obj.relocs.get(sec.index)
                if not rels:
                    continue
                for rel in rels:
                    sym = obj.symbols[rel.sym]
                    nm = sym.name or ""
                    if nm.startswith(FRAME_PREFIX):
                        fn = nm[len(FRAME_PREFIX):]
                        need = (rel.addend & 0xFF) + 2
                        if need > frame_size.get(fn, 0):
                            frame_size[fn] = need
        if not frame_size:
            return   # nothing spills / takes a frame address
        # section base address -> fn-name (a .text.<fn> section), for edges.
        secbase_fn = {}
        fn_of_secid = {}
        for obj in self.objs:
            for sec in obj.sections:
                if classify(sec.name) != "rom" or sec.base < 0:
                    continue
                nm = sec.name
                fn = None
                if nm.startswith(".text."):
                    fn = nm[len(".text."):]
                if fn is not None:
                    secbase_fn[sec.base] = fn
                    fn_of_secid[id(sec)] = fn
        # 2. call-graph edges caller_fn -> callee_fn (ADDR12 between distinct fns)
        succ = {fn: set() for fn in frame_size}
        allfns = set(frame_size)
        for obj in self.objs:
            for sec in obj.sections:
                if classify(sec.name) != "rom" or sec.base < 0:
                    continue
                caller = fn_of_secid.get(id(sec))
                if caller is None:
                    continue
                rels = obj.relocs.get(sec.index)
                if not rels:
                    continue
                for rel in rels:
                    if rel.rtype != R_I4004_ADDR12:
                        continue
                    to = self._target_addr(obj, rel)
                    callee = secbase_fn.get(to) if to is not None else None
                    if callee is not None and callee != caller:
                        # A leaf rt/mem helper (no __frame_ reloc of its own)
                        # holds no frame and, by the leaf-depth invariant
                        # (_assert_leaf_depth), makes no CALLSOFT to a compiled
                        # function -- so it carries no frame liveness between a
                        # caller and a callee. Excluding it as a graph node both
                        # keeps the overlay tight and avoids spurious cycles from
                        # a shared helper that many functions reference.
                        if callee not in frame_size and self._is_leaf_helper(callee):
                            continue
                        if caller not in frame_size and self._is_leaf_helper(caller):
                            continue
                        allfns.add(caller); allfns.add(callee)
                        succ.setdefault(caller, set()).add(callee)
                        succ.setdefault(callee, set())
                        if os.environ.get("I4004_FRAME_EDGES"):
                            import sys as _sys
                            print(f"[edge] {caller} ==> {callee}",
                                  file=_sys.stderr)
        for fn in allfns:
            frame_size.setdefault(fn, 0)
            succ.setdefault(fn, set())
        # 3+4. topological order (Kahn) with cycle detection, then overlay base.
        # 3+4. The reference graph built from ADDR12 edges OVER-approximates
        # calls: it also captures TAIL calls (a `jun callee` that ends the
        # caller, where the caller's frame is already dead) and the central
        # dispatcher pattern where many helpers `jun` back into Calculator
        # ::compute. Those back-edges are NOT frame-liveness edges (the firmware
        # is provably non-recursive in real call depth -- a true cycle would
        # already overflow the hardware/soft return stack). So we break cycles
        # by DROPPING back-edges found in a DFS, leaving a DAG whose edges are
        # the frame-liveness (caller-live-across-callee) edges, then assign
        # base(fn)=max over DAG-predecessors P of (base(P)+size(P)).
        WHITE, GRAY, BLACK = 0, 1, 2
        color = {fn: WHITE for fn in allfns}
        dropped = 0
        dag = {fn: set() for fn in allfns}
        # Seed the DFS from the natural ROOTS (no incoming reference) first, so
        # forward call edges (root -> ... -> leaf) are KEPT and only the true
        # back-edges (a leaf/helper `jun`ing back toward the dispatcher, i.e. a
        # tail call where the caller frame is dead) are dropped. A root-first
        # order makes the kept DAG match the real live-across-call direction.
        orig_indeg = {fn: 0 for fn in allfns}
        for fn in allfns:
            for c in succ[fn]:
                orig_indeg[c] += 1
        roots = sorted(allfns, key=lambda fn: (orig_indeg[fn], fn))
        # iterative DFS to avoid recursion limits on a deep graph
        for root in roots:
            if color[root] != WHITE:
                continue
            stack = [(root, iter(sorted(succ[root])))]
            color[root] = GRAY
            while stack:
                fn, it = stack[-1]
                advanced = False
                for c in it:
                    if color[c] == GRAY:
                        # back-edge -> cycle; drop it (tail call / non-live ref)
                        dropped += 1
                        continue
                    dag[fn].add(c)
                    if color[c] == WHITE:
                        color[c] = GRAY
                        stack.append((c, iter(sorted(succ[c]))))
                        advanced = True
                        break
                if not advanced:
                    color[fn] = BLACK
                    stack.pop()
        if dropped and os.environ.get("I4004_FRAME_DEBUG"):
            import sys as _sys
            print(f"[frames] dropped {dropped} back-edge(s) (tail calls / "
                  f"dispatcher re-entries) to break reference cycles",
                  file=_sys.stderr)
        # topological base assignment over the DAG (Kahn).
        indeg = {fn: 0 for fn in allfns}
        for fn in allfns:
            for c in dag[fn]:
                indeg[c] += 1
        from collections import deque
        q = deque(sorted(fn for fn in allfns if indeg[fn] == 0))
        topo = []
        base = {fn: 0 for fn in allfns}
        while q:
            fn = q.popleft()
            topo.append(fn)
            for c in sorted(dag[fn]):
                if base[fn] + frame_size[fn] > base[c]:
                    base[c] = base[fn] + frame_size[fn]
                indeg[c] -= 1
                if indeg[c] == 0:
                    q.append(c)
        if len(topo) != len(allfns):
            # Should not happen after back-edge removal; guard defensively.
            leftover = sorted(fn for fn in allfns if indeg[fn] > 0)
            raise LinkError(
                "frame overlay: residual cycle after back-edge removal among "
                f"{leftover[:8]}{'...' if len(leftover) > 8 else ''} -- a true "
                "recursion needs an SSP-relative frame (abi.md 4.4).")
        # 5. Place frames in CM-RAM BANK 0 (SRC-addressable low byte), after the
        # linker's data/bss arena and clear of the firmware's reserved working
        # set / I/O chips (handle < 0x20 on the xl board). Frames MUST be in
        # bank 0 because the firmware reaches frame locals through POINTERs
        # (LEA_FI -> LDRAMi8/STRAMi8) that dereference with a plain SRC under the
        # current DCL bank; a frame in another CM-RAM bank would need every such
        # deref to DCL-switch, which the opaque 16-bit pointer handle cannot
        # drive. The soft stack lives in CM-RAM bank 1 (handle 0x100+) on the xl
        # board, so it shares no physical 4002 with a bank-0 frame.
        frame_origin = self.ram_origin + self.ram_used
        if frame_origin < 0x20:
            frame_origin = 0x20
        total = max((base[fn] + frame_size[fn] for fn in allfns), default=0)
        FRAME_RAM_LIMIT = 0x100   # CM-RAM bank 0 addressable SRC chars
        if frame_origin + total > FRAME_RAM_LIMIT:
            worst = max(allfns, key=lambda fn: base[fn] + frame_size[fn])
            raise LinkError(
                f"frame overlay: {len(allfns)} per-function frames need "
                f"{total} chars from 0x{frame_origin:x} (deepest reaches "
                f"0x{frame_origin+total:x} via '{worst}'), exceeding CM-RAM "
                f"bank 0's SRC space (0x{FRAME_RAM_LIMIT:x}). Shrink spilling, "
                f"split functions, or free bank-0 chars.")
        for fn in allfns:
            self.linker_syms[FRAME_PREFIX + fn] = frame_origin + base[fn]
        self.frame_overlay_total = total
        self.frame_overlay_origin = frame_origin
        self.frame_count = len(allfns)
        if os.environ.get("I4004_FRAME_DEBUG"):
            import sys as _sys
            print(f"[frames] {len(allfns)} fns, total {total} chars from "
                  f"0x{frame_origin:x}", file=_sys.stderr)
            for fn in sorted(allfns):
                print(f"  __frame_{fn} = 0x{frame_origin+base[fn]:x} "
                      f"size {frame_size[fn]}", file=_sys.stderr)

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
        elif rt == R_I4004_LO8:
            # low 8 bits of a code symbol into the FIM second word (byte at off)
            self.rom_image[off] = value & 0xFF
        elif rt == R_I4004_PAGE4:
            # page nibble (bits 11..8) of a code symbol into the HIGH nibble of
            # the FIM second word, so FIM Pn,byte puts the page in Pn.hi (R4),
            # which __push_ret reads as the page (abi.md 4.4.1).
            if value > 0xFFF:
                raise LinkError(self._loc(obj, sec, rel) +
                                f": PAGE4 target 0x{value:x} exceeds 12 bits")
            self.rom_image[off] = ((value >> 8) & 0x0F) << 4
        elif rt == R_I4004_BANK4:
            # Low nibble of an LDM operand = the ROM bank the packer placed the
            # referenced code symbol in (far-call forward token). Resolved from
            # the linker's bank assignment, NOT from `value` (an address exists
            # in every bank). Common/linker-defined symbols resolve to bank 0,
            # the caller-independent value the __soft_ret dispatch expects.
            bank = self._sec_bank_of_symbol(obj, rel)
            if bank < 0:
                bank = 0
            self.rom_image[off] = (self.rom_image[off] & 0xF0) | (bank & 0x0F)
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

    # -- banking helpers ----------------------------------------------------
    def _sec_bank_of_symbol(self, obj, rel):
        """Bank of the section a reloc's symbol resolves into (-1 = common /
        linker-defined / RAM)."""
        sym = obj.symbols[rel.sym]
        STT_SECTION = 3
        if (sym.info & 0xf) == STT_SECTION or sym.name == "":
            return self.sec_bank.get(id(obj.sections[sym.shndx]), -1)
        if sym.shndx == 0:
            if sym.name in self.global_syms:
                g_obj, g_sym = self.global_syms[sym.name]
                return self.sec_bank.get(id(g_obj.sections[g_sym.shndx]), -1)
            return -1  # linker-defined
        return self.sec_bank.get(id(obj.sections[sym.shndx]), -1)

    def _target_addr(self, obj, rel):
        """Resolved ADDR12 target address (works for named AND section symbols),
        or None if unresolvable here. Used to key trampolines by target address
        so a cross-firmware call via a .text.<fn>+addend section symbol is
        trampolined too."""
        try:
            return self._sym_addr(obj, rel) + rel.addend
        except LinkError:
            return None

    @staticmethod
    def _tramp_name(addr):
        return f"__bankcall_{addr:04x}"

    def _cross_bank_calls(self):
        """List (loc, from_bank, to_bank, target_addr) for every ADDR12 call
        whose target is in a DIFFERENT bank than the consuming section. Common
        code is mapped in every bank, so common<->* is never cross-bank; only
        banked->other-banked is."""
        out = []
        for obj in self.objs:
            for sec in obj.sections:
                rels = obj.relocs.get(sec.index)
                if not rels or sec.base < 0:
                    continue
                frm = self.sec_bank.get(id(sec), -1)
                for rel in rels:
                    if rel.rtype != R_I4004_ADDR12:
                        continue
                    to = self._sec_bank_of_symbol(obj, rel)
                    if getattr(self, "want_softcall", False):
                        # Soft-call: a call reaching a BANKED callee from a
                        # different bank needs a latch set. This INCLUDES a
                        # common caller (frm == -1): common code runs under
                        # whatever latch is current, so calling a banked callee
                        # must switch to the callee's bank (e.g. crt0's jun main
                        # when main is not in bank 0). A banked->common call is
                        # never cross-bank (common is mapped in every bank).
                        if to != -1 and to != frm:
                            out.append((self._loc(obj, sec, rel), frm, to,
                                        self._target_addr(obj, rel)))
                        continue
                    if frm != -1 and to != -1 and frm != to:
                        out.append((self._loc(obj, sec, rel), frm, to,
                                    self._target_addr(obj, rel)))
        return out

    def cross_bank_callees(self):
        """Map target-address -> its bank, one trampoline per distinct cross-bank
        target address (named or section-symbol callee)."""
        callees = {}
        for _loc, _f, to, addr in self._cross_bank_calls():
            if addr is not None:
                callees[addr] = to
        return callees

    def cross_bank_callee_pairs(self):
        """Soft-call: the set of distinct (target_addr, callee_bank) pairs that
        need a __tobank stub. Keyed by (addr, bank) -- NOT addr alone -- because
        a banked PC (e.g. 0x800) exists in EVERY bank, so two callees in
        different banks can share an address and still need separate stubs."""
        pairs = {}
        for _loc, _f, to, addr in self._cross_bank_calls():
            if addr is not None and to != -1:
                pairs[(addr, to)] = to
        return pairs

    def emit_trampolines_source(self):
        """Generate trampolines.s: one __bankcall_<addr> per distinct cross-bank
        TARGET ADDRESS, in the common region, setting the latch to the target's
        bank around a JMS to the target, saving/restoring the active bank on the
        P7 soft stack (via the __cur_bank RAM cell). Preserves all arg/return
        registers (only P0/P7/ACC touched). The JMS target is the absolute
        banked address (a `.set`), since the packer has already resolved it and
        the callee may be an anonymous .text section + addend."""
        callees = self.cross_bank_callees()
        cur = self.cur_bank_src()
        lines = [
            "; GENERATED by i4004_pack.py --emit-trampolines. One __bankcall_<addr>",
            "; per distinct cross-bank TARGET; see memory_map.md 'ROM bank switching'.",
        ]
        for addr, bank in sorted(callees.items()):
            tname = self._tramp_name(addr)
            tgt = f"__bt_{addr:04x}"
            lines += [
                f"\t.set\t{tgt}, 0x{addr:x}",
                f"\t.section\t.text.{tname},\"ax\"",
                f"\t.globl\t{tname}",
                f"{tname}:",
                f"\tfim\tp0, 0x{cur:02x}",   # __cur_bank SRC addr
                "\tsrc\tp0",
                "\trdm",                      # ACC = current bank
                "\tsrc\tp7",
                "\twrm",                      # push current bank at SSP
                "\tinc\tr15",
                f"\tldm\t{bank & 0xf}",
                "\tfim\tp0, 0x30",            # ROM chip 3 latch port
                "\tsrc\tp0",
                "\twrr",                      # latch <- bank
                f"\tldm\t{bank & 0xf}",
                f"\tfim\tp0, 0x{cur:02x}",
                "\tsrc\tp0",
                "\twrm",                      # __cur_bank <- bank
                f"\tjms\t{tgt}",             # call the now-mapped target (abs addr)
                "\tld\tr15",
                "\tdac",
                "\txch\tr15",                 # SSP--
                "\tsrc\tp7",
                "\trdm",                      # ACC = old bank
                "\tfim\tp0, 0x30",
                "\tsrc\tp0",
                "\twrr",                      # latch <- old bank
                "\tsrc\tp7",
                "\trdm",
                f"\tfim\tp0, 0x{cur:02x}",
                "\tsrc\tp0",
                "\twrm",                      # __cur_bank <- old bank
                "\tbbl\t0",
            ]
        return "\n".join(lines) + "\n"

    def cur_bank_src(self):
        """SRC address byte of the __cur_bank cell. Chip 0 reg 0 char 15 (0x0f)
        -- a scratch char the firmware's working set does not use (digits use
        chars 0..9 of a register; char 15 of reg0 is free). Documented in
        memory_map.md."""
        return 0x0F

    def _build_banked_images(self):
        """Return a list of per-bank 4096-byte ROM arrays. Bank k = the common
        region (identical everywhere) + bank k's banked sections, all
        relocations resolved (every placed section's base is its final PC)."""
        images = [bytearray(self.common_bytes + self.bank_bytes)
                  for _ in range(self.banks)]
        # place section bytes
        for p in self.placed:
            if p.kind != "rom":
                continue
            b = self.sec_bank.get(id(p.sec), -1)
            targets = range(self.banks) if b == -1 else [b]
            for k in targets:
                images[k][p.base:p.base + p.sec.size] = p.sec.data
        # resolve relocations into each bank image that contains the section
        for obj in self.objs:
            for sec in obj.sections:
                rels = obj.relocs.get(sec.index)
                if not rels or sec.base < 0 or classify(sec.name) != "rom":
                    continue
                b = self.sec_bank.get(id(sec), -1)
                banks = range(self.banks) if b == -1 else [b]
                for k in banks:
                    self.rom_image = images[k]       # _patch writes self.rom_image
                    for rel in rels:
                        # Far-call lowering (abi.md 4.4.3): redirect a cross-bank
                        # `jun callee` to the in-bank forward-token thunk, which
                        # pushes (off, page, callee_bank) and `jun __soft_ret`.
                        # The thunk lives in the CALLER's bank (host_bank) and
                        # never touches the latch, so no running instruction is
                        # swapped out. A COMMON caller (b == -1) hosts its thunk
                        # in the callee bank; such a common call site is only
                        # rewritten in the image for that bank (k == host_bank),
                        # which is where control actually is when it fires
                        # (crt0's `jun main` runs after the latch already points
                        # at main's bank via the reset default / prior switch).
                        if (getattr(self, "want_softcall", False)
                                and getattr(self, "far_calls", True)
                                and rel.rtype == R_I4004_ADDR12):
                            to = self._sec_bank_of_symbol(obj, rel)
                            addr = self._target_addr(obj, rel)
                            if to != -1 and to != b and addr is not None:
                                if b == -1:
                                    # common caller -> common thunk (mapped in
                                    # every bank image; rewrite everywhere).
                                    th = self.far_thunks_common.get((addr, to))
                                    if th is not None:
                                        self._patch(obj, sec, sec.base, rel, th[0])
                                        continue
                                else:
                                    # banked caller -> in-bank thunk (host==b==k)
                                    # or a common-fallback thunk (mapped in every
                                    # bank image when the bank tail was full).
                                    # Key includes the CALLEE bank `to`: a banked
                                    # PC is identical in every bank, so two
                                    # distinct callees can share `addr` and must
                                    # not share a thunk (deep-chain N-2 bug).
                                    th = self.far_thunks.get((addr, to, b))
                                    if th is not None:
                                        is_common = th[3]
                                        if is_common or k == b:
                                            self._patch(obj, sec, sec.base, rel, th[0])
                                            continue
                        # Legacy soft-call __tobank stub (when --no-far-calls).
                        if (getattr(self, "want_softcall", False)
                                and not getattr(self, "far_calls", True)
                                and rel.rtype == R_I4004_ADDR12):
                            to = self._sec_bank_of_symbol(obj, rel)
                            addr = self._target_addr(obj, rel)
                            if (to != -1 and to != b and addr is not None
                                    and (addr, to) in getattr(self, "bank_stubs", {})):
                                stub_base = self.bank_stubs[(addr, to)]
                                self._patch(obj, sec, sec.base, rel, stub_base)
                                continue
                        # Legacy __bankcall_* trampoline rewrite (non-softcall).
                        if (self.rewrite_cross_bank and rel.rtype == R_I4004_ADDR12
                                and b != -1):
                            to = self._sec_bank_of_symbol(obj, rel)
                            addr = self._target_addr(obj, rel)
                            if to != -1 and to != b and addr is not None:
                                tname = self._tramp_name(addr)
                                if tname in self.global_syms:
                                    g_obj, g_sym = self.global_syms[tname]
                                    S = g_obj.sections[g_sym.shndx].base + g_sym.value
                                    # trampoline carries the real target; the
                                    # call site jumps to the trampoline (no addend)
                                    self._patch(obj, sec, sec.base, rel, S)
                                    continue
                        S = self._sym_addr(obj, rel)
                        self._patch(obj, sec, sec.base, rel, S + rel.addend)
        return images

    # -- driver -------------------------------------------------------------
    def link(self):
        self._collect_globals()
        self._place_rom()
        self._place_ram()
        self._linker_defined()
        self._alloc_frames()
        if self.banks:
            if getattr(self, "want_softcall", False):
                # Soft-call banking: the return dispatch (__soft_ret, push
                # helpers, __srtab, per-page jin-p0 stubs) lives in the common
                # region, mapped identically in every bank. A compiled call is a
                # CALLSOFT `jun callee` to the callee's common PC. The cross-bank
                # FORWARD switch is handled by per-callee common `__tobank_<addr>`
                # set-latch stubs (set latch + __cur_bank to the callee bank,
                # then jun), and the RETURN restore is the token's 4th bank
                # nibble written back to the latch + __cur_bank in __soft_ret.
                # The old __bankcall_* JMS trampolines are not used.
                self._synth_softcall_banked()
                self.bank_images = self._build_banked_images()
                for k in range(self.banks):
                    self._write_softcall(self.bank_images[k], self.n_pages)
                    if getattr(self, "far_calls", True):
                        self._write_far_thunks(self.bank_images[k], k)
                    else:
                        self._write_bank_stubs(self.bank_images[k])
                self._assert_leaf_depth()
                return
            xb = self._cross_bank_calls()
            if xb and not self.rewrite_cross_bank:
                callees = sorted(self.cross_bank_callees())
                raise LinkError(
                    f"{len(xb)} cross-bank ADDR12 call(s) to {len(callees)} "
                    f"callee(s) need trampolines. Run --emit-trampolines to "
                    f"generate __bankcall_* in a trampolines.s, assemble it, and "
                    f"re-link that object in with --rewrite-cross-bank.")
            if xb and self.rewrite_cross_bank:
                # every cross-bank TARGET must have a __bankcall_<addr> trampoline
                missing = [self._tramp_name(a) for a in self.cross_bank_callees()
                           if self._tramp_name(a) not in self.global_syms]
                if missing:
                    raise LinkError(
                        "--rewrite-cross-bank: missing trampolines for "
                        + ", ".join(missing[:10])
                        + " (re-run --emit-trampolines and link the .o)")
            self.bank_images = self._build_banked_images()
        else:
            self._synth_softcall()
            self._apply()
            self._emit_softcall()
            self._assert_leaf_depth()

    def rom_bin(self, pad_to=None):
        if self.banks:
            # image = common(common_bytes) || bank_0_hi(bank_bytes) || ...
            # bank k image already has common in [0,common_bytes); take its
            # common once, then each bank's banked tail.
            out = bytearray(self.bank_images[0][:self.common_bytes])
            for k in range(self.banks):
                out.extend(self.bank_images[k][self.common_bytes:
                                               self.common_bytes + self.bank_bytes])
            return bytes(out)
        out = bytearray(self.rom_image)
        if pad_to is not None and len(out) < pad_to:
            out.extend(b"\x00" * (pad_to - len(out)))
        return bytes(out)

    def mapfile(self) -> str:
        lines = []
        lines.append("i4004 image map")
        lines.append("=" * 60)
        if self.banks:
            total = self.common_bytes + self.banks * self.bank_bytes
            lines.append(f"BANKED image: {self.banks} banks, common 0x{self.common_bytes:x} "
                         f"+ {self.banks} x 0x{self.bank_bytes:x} = {total} bytes total")
            lines.append(f"  common used: {self.common_used}/{self.common_bytes} bytes")
            for k in range(self.banks):
                lines.append(f"  bank {k}: {self.bank_fill[k]}/{self.bank_bytes} bytes")
            lines.append("  Function banks:")
            for p in sorted(self.placed, key=lambda q: (self.sec_bank.get(id(q.sec), -1), q.base)):
                if p.kind != "rom":
                    continue
                b = self.sec_bank.get(id(p.sec), -1)
                tag = "common" if b == -1 else f"bank{b}"
                lines.append(f"    0x{p.base:03x} +{p.sec.size:<4} {tag:7} "
                             f"{p.obj.path.split('/')[-1]}:{p.sec.name}")
            lines.append("")
            return "\n".join(lines) + "\n"
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
    ap.add_argument("-o", "--output", required=False, help="output ROM .bin")
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
    ap.add_argument("--banks", type=int, default=0,
                    help="xl bank-switched ROM: number of banks (0 = flat, default). "
                         "Image = common(--common-bytes) || bank_k(--bank-bytes) per bank.")
    ap.add_argument("--common-bytes", type=lambda s: int(s, 0), default=0x400,
                    help="size of the always-mapped common region (default 0x400 = chips 0-3)")
    ap.add_argument("--bank-bytes", type=lambda s: int(s, 0), default=0xC00,
                    help="size of each bank's banked region (default 0xC00 = chips 4-15)")
    ap.add_argument("--emit-trampolines", metavar="FILE",
                    help="pass 1: place functions into banks, then WRITE the "
                         "__bankcall_* trampoline source for every cross-bank "
                         "callee to FILE and exit (no image emitted). Assemble "
                         "FILE and re-link with --rewrite-cross-bank.")
    ap.add_argument("--rewrite-cross-bank", action="store_true",
                    help="pass 2: rewrite cross-bank ADDR12 calls to their "
                         "__bankcall_* trampoline (the trampolines.o must be "
                         "among the input objects).")
    ap.add_argument("--softcall", action="store_true",
                    help="synthesize the software-call return dispatch "
                         "(__soft_ret + __srtab + per-page jin-p0 stubs) and "
                         "assert the JMS leaf-depth rule (abi.md 4.4.1/4.4.2).")
    ap.add_argument("--far-calls", dest="far_calls", action="store_true",
                    default=True,
                    help="(banked --softcall, DEFAULT ON) lower each cross-bank "
                         "call to an in-caller-bank forward-token thunk + "
                         "`jun __soft_ret`, removing the common __tobank stubs "
                         "(abi.md 4.4.3).")
    ap.add_argument("--no-far-calls", dest="far_calls", action="store_false",
                    help="restore the legacy common __tobank set-latch stubs "
                         "instead of far-call thunks (for A/B comparison).")
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

    if args.banks == 0 and rom_len > 4096:
        print(f"ERROR: ROM budget {rom_len} exceeds 4004 architectural max 4096",
              file=sys.stderr)
        return 2
    if args.banks:
        # banking: per-bank view is common+bank <= 4096; total ROM is larger.
        rom_len = args.common_bytes + args.bank_bytes

    try:
        objs = [read_elf(p) for p in args.objects]
        ln = Linker(objs, rom_len, ram_origin, ram_len, stack_origin, stack_len,
                    args.pad_pages, args.allow_eop_rollforward,
                    banks=args.banks, common_bytes=args.common_bytes,
                    bank_bytes=args.bank_bytes)
        ln.rewrite_cross_bank = args.rewrite_cross_bank
        ln.want_softcall = args.softcall
        ln.far_calls = args.far_calls
        if args.emit_trampolines:
            # pass 1: placement only, then write the trampoline source.
            if not args.banks:
                print("ERROR: --emit-trampolines requires --banks", file=sys.stderr)
                return 2
            ln._collect_globals()
            ln._place_rom()
            ln._place_ram()
            ln._linker_defined()
            with open(args.emit_trampolines, "w") as fh:
                fh.write(ln.emit_trampolines_source())
            n = len(ln.cross_bank_callees())
            print(f"wrote {args.emit_trampolines}: {n} cross-bank trampolines",
                  file=sys.stderr)
            return 0
        if not args.output:
            print("ERROR: -o/--output is required (unless --emit-trampolines)",
                  file=sys.stderr)
            return 2
        ln.link()
        pad_to = args.pad
        binimg = ln.rom_bin(pad_to=pad_to)
        with open(args.output, "wb") as fh:
            fh.write(binimg)
        if args.mapfile:
            with open(args.mapfile, "w") as fh:
                fh.write(ln.mapfile())
        print(f"wrote {args.output}: {len(binimg)} bytes", file=sys.stderr)
    except LinkError as e:
        print(f"i4004_pack: link error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
