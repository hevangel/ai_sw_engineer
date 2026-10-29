#!/usr/bin/env python3
"""i8008_link.py - the llvm-i8008 linker (spec/codegen.md sec.6, abi.md sec.4-5).

Reads ELF32 objects from llc / llvm-mc (EM_I8008), places their sections at
the addresses a small layout file gives, allocates the static frames and
memory-argument areas by call-graph overlay, checks the call depth against the
8008's 7-level stack, applies R_I8008_* relocations and writes a raw image.

Layout file (one directive per line, '#' comments):

    CODE  <start> <size>      .text* and .rodata* (ROM or RAM)
    DATA  <start> <size>      .data* .bss* frames, argument areas, __icall
    DATA  follow <size>       DATA starts right after CODE, in the same image
    ENTRY <symbol>            placed first in CODE; root for --gc-sections
    KEEP  <symbol>            extra root
    PLACE <symbol> <addr>     put the section defining <symbol> at <addr> (a
                              fixed entry point); the rest of CODE flows around
    DEPTH <n>                 maximum call depth (default 7)
    RESET <symbol>            a restart point: code jumps or calls to it from
                              any depth and never returns (the 8008's stack
                              is circular, so the return addresses left
                              behind are harmless). Calls to it are not call
                              graph edges; it is a root of its own, like ENTRY
    DATAPAGE                  .data, .bss, frames and argument areas must share
                              one 256-byte page (the image was compiled with
                              llc -i8008-data-page)

The image covers CODE, and also DATA's .data/.bss when DATA follows CODE.
With a separate DATA region the image holds CODE only; .data is then an error
(nothing would initialise it) and the program's crt0 zeroes .bss between
__bss_start and __bss_end.
"""
from __future__ import annotations

import argparse
import pathlib
import struct
import sys
from dataclasses import dataclass, field

EM_I8008 = 0x8008
SHT_PROGBITS, SHT_SYMTAB, SHT_RELA, SHT_NOBITS = 1, 2, 4, 8
SHF_ALLOC = 0x2


def safe_out(path):
    """CLI output path: relative paths must stay inside the working
    directory (no `..` climbs); absolute paths are explicit intent."""
    p = pathlib.Path(path).resolve()
    if not pathlib.Path(path).is_absolute() and pathlib.Path.cwd() not in p.parents:
        raise SystemExit(f"i8008_link: refusing relative output outside the "
                         f"working directory: {path}")
    return p
SHN_UNDEF, SHN_ABS, SHN_COMMON = 0, 0xFFF1, 0xFFF2
R_NONE, R_16, R_LO8, R_HI8, R_8 = 0, 1, 2, 3, 4
STT_FUNC = 2
PAGE = 256
ADDR_MAX = 0x4000


class LinkError(Exception):
    pass


@dataclass
class Section:
    name: str
    sh_type: int
    flags: int
    size: int
    align: int
    index: int
    data: bytes
    obj: "Obj" = None
    addr: int = -1
    live: bool = False

    @property
    def key(self):
        return (id(self.obj), self.index)


@dataclass
class Symbol:
    name: str
    value: int
    size: int
    info: int
    shndx: int

    @property
    def is_global(self):
        return (self.info >> 4) != 0  # GLOBAL or WEAK


@dataclass
class Reloc:
    offset: int
    sym: int
    rtype: int
    addend: int


@dataclass
class Obj:
    path: str
    sections: list = field(default_factory=list)
    symbols: list = field(default_factory=list)
    relocs: dict = field(default_factory=dict)


def _cstr(buf, off):
    return buf[off:buf.find(b"\0", off)].decode("ascii", "replace")


def read_elf(path):
    data = open(path, "rb").read()
    if data[:4] != b"\x7fELF" or data[4] != 1 or data[5] != 1:
        raise LinkError(f"{path}: not an ELF32 little-endian file")
    (_, e_machine, _, _, _, e_shoff, _, _, _, _, e_shentsize, e_shnum,
     e_shstrndx) = struct.unpack_from("<HHIIIIIHHHHHH", data, 16)
    if e_machine != EM_I8008:
        raise LinkError(f"{path}: e_machine 0x{e_machine:x}, expected 0x{EM_I8008:x}")
    raw = [struct.unpack_from("<IIIIIIIIII", data, e_shoff + i * e_shentsize)
           for i in range(e_shnum)]
    shstr = data[raw[e_shstrndx][4]:raw[e_shstrndx][4] + raw[e_shstrndx][5]]
    obj = Obj(path)
    for i, (nm, typ, flg, _, off, size, link, info, align, _) in enumerate(raw):
        sdata = b"" if typ == SHT_NOBITS else data[off:off + size]
        s = Section(_cstr(shstr, nm), typ, flg, size, max(align, 1), i, sdata, obj)
        s.link, s.info = link, info
        obj.sections.append(s)
    for s in obj.sections:
        if s.sh_type == SHT_SYMTAB:
            strtab = obj.sections[s.link].data
            for k in range(s.size // 16):
                n, v, sz, info, _, shndx = struct.unpack_from("<IIIBBH", s.data, k * 16)
                obj.symbols.append(Symbol(_cstr(strtab, n), v, sz, info, shndx))
        elif s.sh_type == SHT_RELA:
            for k in range(s.size // 12):
                off, inf, add = struct.unpack_from("<IIi", s.data, k * 12)
                obj.relocs.setdefault(s.info, []).append(Reloc(off, inf >> 8, inf & 0xFF, add))
    return obj


def parse_num(s):
    return int(s, 0)


@dataclass
class Layout:
    code: tuple = (0, ADDR_MAX)
    data: tuple = None          # (start, size) or ("follow", size)
    entry: str = None
    keep: list = field(default_factory=list)
    reset: list = field(default_factory=list)
    place: dict = field(default_factory=dict)
    depth: int = 7
    data_page: bool = False


def read_layout(path):
    lay = Layout()
    for ln, line in enumerate(open(path), 1):
        line = line.split("#", 1)[0].split()
        if not line:
            continue
        d, args = line[0].upper(), line[1:]
        if d == "CODE":
            lay.code = (parse_num(args[0]), parse_num(args[1]))
        elif d == "DATA":
            lay.data = ("follow" if args[0] == "follow" else parse_num(args[0]),
                        parse_num(args[1]))
        elif d == "ENTRY":
            lay.entry = args[0]
        elif d == "KEEP":
            lay.keep.append(args[0])
        elif d == "PLACE":
            lay.place[args[0]] = parse_num(args[1])
            lay.keep.append(args[0])
        elif d == "RESET":
            lay.reset.append(args[0])
            lay.keep.append(args[0])
        elif d == "DATAPAGE":
            lay.data_page = True
        elif d == "DEPTH":
            lay.depth = parse_num(args[0])
        else:
            raise LinkError(f"{path}:{ln}: unknown directive {d}")
    return lay


def kind(name):
    if name.startswith(".text"):
        return "text"
    if name.startswith(".rodata"):
        return "rodata"
    if name.startswith(".data"):
        return "data"
    if name.startswith(".bss"):
        return "bss"
    return None


class Linker:
    def __init__(self, objs, layout, gc=True):
        self.objs, self.lay, self.gc = objs, layout, gc
        self.defs = {}       # global name -> (obj, Symbol)
        self.abs = {}        # name -> value (absolute / linker defined)
        self.extra = {}      # linker-made areas: name -> (addr, size)
        self.sizes = {}      # name -> bytes, from __framesize_/__argsize_
        self.map_lines = []

    # ---------------------------------------------------------------- symbols
    def collect(self):
        for o in self.objs:
            for s in o.symbols:
                if not s.name or not s.is_global or s.shndx == SHN_UNDEF:
                    continue
                if s.shndx == SHN_COMMON:
                    raise LinkError(f"{o.path}: common symbol {s.name} unsupported")
                if s.name in self.defs or s.name in self.abs:
                    raise LinkError(f"duplicate symbol {s.name}")
                if s.shndx == SHN_ABS:
                    self.abs[s.name] = s.value
                else:
                    self.defs[s.name] = (o, s)
        for name, v in self.abs.items():
            for pre in ("__framesize_", "__argsize_"):
                if name.startswith(pre):
                    self.sizes[name] = v

    def target_section(self, o, rel):
        s = o.symbols[rel.sym]
        if s.shndx not in (SHN_UNDEF, SHN_ABS, SHN_COMMON) and not (s.is_global and s.name):
            return o.sections[s.shndx]
        if s.name in self.defs:
            do, ds = self.defs[s.name]
            return do.sections[ds.shndx]
        return None

    def sym_name(self, o, rel):
        return o.symbols[rel.sym].name

    # ------------------------------------------------------------ gc-sections
    def alloc_sections(self):
        return [s for o in self.objs for s in o.sections
                if s.flags & SHF_ALLOC and s.size and kind(s.name)]

    def mark_live(self):
        secs = self.alloc_sections()
        if not self.gc:
            for s in secs:
                s.live = True
            return
        roots = [r for r in [self.lay.entry] + self.lay.keep if r]
        work = []
        for r in roots:
            if r not in self.defs:
                raise LinkError(f"root symbol {r} is not defined")
            o, s = self.defs[r]
            work.append(o.sections[s.shndx])
        while work:
            sec = work.pop()
            if sec.live:
                continue
            sec.live = True
            for rel in sec.obj.relocs.get(sec.index, []):
                t = self.target_section(sec.obj, rel)
                if t is not None and not t.live and kind(t.name):
                    work.append(t)

    # ---------------------------------------------------------------- layout
    def place(self):
        live = [s for s in self.alloc_sections() if s.live]
        entry_sec = None
        if self.lay.entry:
            o, s = self.defs[self.lay.entry]
            entry_sec = o.sections[s.shndx]
        code = [s for s in live if kind(s.name) in ("text", "rodata")]
        code.sort(key=lambda s: 0 if s is entry_sec else 1)
        start, size = self.lay.code
        pinned = []
        for sym, at in self.lay.place.items():
            o, ps = self.defs[sym]
            sec = o.sections[ps.shndx]
            if ps.value:
                raise LinkError(f"PLACE {sym}: not at the start of its section")
            sec.addr = at
            pinned.append((at, at + sec.size, sec))
        # Free intervals of CODE around the pinned sections. The ENTRY section
        # (unless pinned) goes first, at the start; the rest best-fit,
        # largest first, so the holes the pinned entries leave get filled.
        free = []
        lo = start
        for a, b, _ in sorted(pinned, key=lambda p: p[0]):
            if a > lo:
                free.append([lo, a])
            lo = max(lo, b)
        free.append([lo, start + size])
        rest = [s for s in code if not any(s is p[2] for p in pinned)]
        first = [s for s in rest if s is entry_sec]
        rest = first + sorted((s for s in rest if s is not entry_sec),
                              key=lambda s: -s.size)
        overflow = 0
        for s in rest:
            best = None
            for iv in free:
                a = -(-iv[0] // s.align) * s.align
                room = iv[1] - a
                if room >= s.size and (s is entry_sec or best is None or room < best[1]):
                    best = (iv, room, a)
                    if s is entry_sec:
                        break
            if s is entry_sec and (best is None or best[2] != start):
                raise LinkError(f"ENTRY {self.lay.entry}: does not fit at the start of CODE")
            if best is None:
                overflow += s.size
                s.addr = start + size
                continue
            iv, _, a = best
            s.addr = a
            iv[0] = a + s.size
        if overflow:
            used = sum(s.size for s in code)
            raise LinkError(f"CODE overflow: 0x{used:x} bytes of code, {overflow} B do not fit "
                            f"in 0x{size:x}")
        pc = max([start] + [s.addr + s.size for s in code])
        self.code_end = pc

        if self.lay.data is None:
            dstart, dsize = pc, ADDR_MAX - pc
            self.follow = True
        elif self.lay.data[0] == "follow":
            dstart, dsize = pc, self.lay.data[1]
            self.follow = True
        else:
            dstart, dsize = self.lay.data
            self.follow = False
        self.data_start, self.data_limit = dstart, dstart + dsize
        dp = dstart

        def put(n, al=1):
            nonlocal dp
            dp = -(-dp // al) * al
            if n <= PAGE and dp // PAGE != (dp + n - 1) // PAGE:
                dp = -(-dp // PAGE) * PAGE   # objects <= 256 B never cross a page
            a = dp
            dp += n
            return a

        data = [s for s in live if kind(s.name) == "data"]
        bss = [s for s in live if kind(s.name) == "bss"]
        if data and not self.follow:
            raise LinkError(".data in a ROM layout: " +
                            ", ".join(s.name for s in data) + " (make it const or zeroed)")
        for s in data:
            s.addr = put(s.size, s.align)
        self.bss_start = dp
        for s in bss:
            s.addr = put(s.size, s.align)
        self.bss_end = dp
        # __icall trampoline: 3 bytes in one page (abi.md sec.3.4).
        if self.referenced("__icall"):
            self.extra["__icall"] = (put(3), 3)
        self.frames_start = dp
        self.dp = dp
        self.put = put

    def referenced(self, name):
        return any(self.sym_name(o, r) == name
                   for o in self.objs for s in o.sections if s.live
                   for r in o.relocs.get(s.index, []))

    # ------------------------------------------------------------ call graph
    def func_of_section(self):
        """A code section's function: its STT_FUNC symbol (compiled code), or
        else the named symbol at offset 0 (hand-written asm has untyped
        labels). Each function sits in its own section (-function-sections,
        and one `.section .text.<name>` per routine in asm)."""
        m = {}
        for o in self.objs:
            for s in o.symbols:
                if s.name and (s.info & 0xF) == STT_FUNC and s.shndx not in (SHN_UNDEF, SHN_ABS):
                    m.setdefault((id(o), s.shndx), s.name)
            for s in o.symbols:
                if (s.name and s.value == 0 and (s.info & 0xF) != 3  # not STT_SECTION
                        and s.shndx not in (SHN_UNDEF, SHN_ABS, SHN_COMMON)
                        and kind(o.sections[s.shndx].name) == "text"):
                    m.setdefault((id(o), s.shndx), s.name)
        return m

    def call_graph(self):
        fos = self.func_of_section()
        calls, taken, tails = {}, set(), set()
        icall_users = set()
        for o in self.objs:
            for sec in o.sections:
                if not sec.live or kind(sec.name) != "text":
                    continue
                caller = fos.get((id(o), sec.index), sec.name)
                calls.setdefault(caller, set())
                for rel in o.relocs.get(sec.index, []):
                    name = self.sym_name(o, rel)
                    t = self.target_section(o, rel)
                    tf = fos.get((id(t.obj), t.index)) if t is not None else None
                    if tf in self.lay.reset:
                        calls.setdefault(tf, set())
                        continue                    # a restart, not a call
                    if rel.rtype == R_16 and rel.offset >= 1:
                        op = sec.data[rel.offset - 1]
                        is_call = op == 0o106 or (op & 0o307) == 0o102
                        if is_call:
                            if name == "__icall":
                                icall_users.add(caller)
                            elif tf:
                                calls[caller].add(tf)
                            continue
                        if tf and tf != caller and (op & 0o307) == 0o104:
                            calls[caller].add(tf)   # a jump into another function:
                            tails.add((caller, tf))  # no stack level of its own
                            continue
                    if tf and t is not None and kind(t.name) == "text" and tf != caller:
                        taken.add(tf)               # address taken
        for u in icall_users:
            calls[u] |= taken
        self.calls, self.taken, self.tails = calls, taken, tails
        return calls

    def check_depth(self):
        calls = self.call_graph()
        depth, onstack = {}, set()

        def d(f, path):
            if f in depth:
                return depth[f]
            if f in onstack:
                raise LinkError("recursion: " + " -> ".join(path + [f]))
            onstack.add(f)
            v = 0
            for c in calls.get(f, ()):
                v = max(v, (0 if (f, c) in self.tails else 1) + d(c, path + [f]))
            onstack.discard(f)
            depth[f] = v
            return v

        roots = [self.lay.entry] + self.lay.reset if self.lay.entry else list(calls)
        worst = max((d(r, []) for r in roots if r), default=0)
        for f in calls:
            d(f, [])
        self.depth = depth
        if worst > self.lay.depth:
            chain = [max((r for r in roots if r), key=lambda r: depth.get(r, 0))]
            while calls.get(chain[-1]):
                chain.append(max(calls[chain[-1]], key=lambda c: depth.get(c, 0)))
            raise LinkError(f"call depth {worst} > {self.lay.depth}: " + " -> ".join(chain))
        return worst

    # --------------------------------------------------------------- frames
    def alloc_frames(self):
        """Overlay: a function's area sits above the areas of every function
        that can be live while it runs (its callers, transitively)."""
        need = {}
        for name in self.undefined():
            for pre in ("__frame_", "__args_"):
                if name.startswith(pre):
                    fn = name[len(pre):]
                    sz = self.sizes.get(("__framesize_" if pre == "__frame_" else "__argsize_") + fn)
                    if sz is None:
                        # An asm routine's argument area: as large as any
                        # caller's store into it.
                        sz = self.max_offset(name) + 1
                    need.setdefault(fn, {})[pre] = sz
        callers = {}
        for f, cs in self.calls.items():
            for c in cs:
                callers.setdefault(c, set()).add(f)
        base, done = {}, {}
        order = []

        def visit(f, seen=()):
            if f in done:
                return
            for p in callers.get(f, ()):
                if p not in seen:
                    visit(p, seen + (f,))
            done[f] = True
            order.append(f)

        for f in set(need) | set(self.calls):
            visit(f)
        top = {}
        for f in order:
            lo = max([top.get(p, self.frames_start) for p in callers.get(f, ())] +
                     [self.frames_start])
            p = lo
            for pre in ("__args_", "__frame_"):
                sz = need.get(f, {}).get(pre, 0)
                if not sz:
                    continue
                if sz > PAGE:
                    raise LinkError(f"{pre}{f}: {sz} bytes > {PAGE} (one page)")
                if p // PAGE != (p + sz - 1) // PAGE:
                    p = -(-p // PAGE) * PAGE
                self.extra[pre + f] = (p, sz)
                p += sz
            top[f] = p
        self.frames_end = max([self.frames_start] + list(top.values()))
        if self.lay.data_page and self.frames_end > self.data_start and                 self.data_start // PAGE != (self.frames_end - 1) // PAGE:
            raise LinkError(f"DATAPAGE: data 0x{self.data_start:04x}..0x{self.frames_end:04x} "
                            "is not within one page")
        if self.frames_end > self.data_limit:
            raise LinkError(f"DATA overflow: frames end at 0x{self.frames_end:04x} > "
                            f"0x{self.data_limit:04x}")

    def undefined(self):
        names = set()
        for o in self.objs:
            for s in o.sections:
                if not s.live:
                    continue
                for r in o.relocs.get(s.index, []):
                    sym = o.symbols[r.sym]
                    if sym.shndx == SHN_UNDEF and sym.name not in self.defs:
                        names.add(sym.name)
        return names

    def max_offset(self, name):
        m = 0
        for o in self.objs:
            for s in o.sections:
                if s.live:
                    for r in o.relocs.get(s.index, []):
                        if self.sym_name(o, r) == name:
                            m = max(m, r.addend + (1 if r.rtype != R_16 else 1))
        return m

    # ----------------------------------------------------------- relocation
    def value(self, o, rel):
        s = o.symbols[rel.sym]
        if s.shndx not in (SHN_UNDEF, SHN_ABS, SHN_COMMON) and not (s.is_global and s.name in self.defs):
            sec = o.sections[s.shndx]
            if sec.addr < 0:
                raise LinkError(f"{o.path}: reference to dropped section {sec.name}")
            return sec.addr + s.value + rel.addend
        if s.shndx == SHN_ABS:
            return s.value + rel.addend
        n = s.name
        if n in self.defs:
            do, ds = self.defs[n]
            sec = do.sections[ds.shndx]
            return sec.addr + ds.value + rel.addend
        if n in self.extra:
            return self.extra[n][0] + rel.addend
        if n in self.abs:
            return self.abs[n] + rel.addend
        special = {"__bss_start": self.bss_start, "__bss_end": self.bss_end,
                   "__frames_end": self.frames_end, "__data_start": self.data_start}
        if n in special:
            return special[n] + rel.addend
        raise LinkError(f"{o.path}: undefined symbol {n}")

    def build_image(self):
        lo = self.lay.code[0]
        hi = self.bss_end if self.follow else self.code_end
        img = bytearray(max(0, hi - lo))
        for o in self.objs:
            for s in o.sections:
                if not s.live:
                    continue
                buf = bytearray(s.data) if s.sh_type != SHT_NOBITS else bytearray(s.size)
                for r in o.relocs.get(s.index, []):
                    v = self.value(o, r)
                    where = f"{o.path}:{s.name}+0x{r.offset:x} ({self.sym_name(o, r)})"
                    if r.rtype == R_16:
                        if not 0 <= v < ADDR_MAX:
                            raise LinkError(f"{where}: address 0x{v:x} out of range")
                        buf[r.offset:r.offset + 2] = struct.pack("<H", v)
                    elif r.rtype == R_LO8:
                        buf[r.offset] = v & 0xFF
                    elif r.rtype == R_HI8:
                        buf[r.offset] = (v >> 8) & 0xFF
                    elif r.rtype == R_8:
                        if not -128 <= v <= 255:
                            raise LinkError(f"{where}: value {v} does not fit a byte")
                        buf[r.offset] = v & 0xFF
                    elif r.rtype != R_NONE:
                        raise LinkError(f"{where}: unknown relocation {r.rtype}")
                if kind(s.name) == "bss" or (not self.follow and kind(s.name) not in ("text", "rodata")):
                    continue
                img[s.addr - lo:s.addr - lo + s.size] = buf
        return bytes(img)

    def link(self):
        self.collect()
        self.mark_live()
        self.place()
        worst = self.check_depth()
        self.alloc_frames()
        img = self.build_image()
        self.worst = worst
        return img

    def map_text(self):
        out = [f"image 0x{self.lay.code[0]:04x}..0x{(self.bss_end if self.follow else self.code_end):04x}",
               f"code  0x{self.lay.code[0]:04x}..0x{self.code_end:04x} ({self.code_end - self.lay.code[0]} B)",
               f"data  0x{self.data_start:04x}..0x{self.frames_end:04x} "
               f"(bss 0x{self.bss_start:04x}..0x{self.bss_end:04x}, frames to 0x{self.frames_end:04x})",
               f"call depth {self.worst} (limit {self.lay.depth})", "", "sections:"]
        for o in self.objs:
            for s in sorted((s for s in o.sections if s.live), key=lambda s: s.addr):
                out.append(f"  0x{s.addr:04x} {s.size:5d}  {s.name}  [{o.path.split('/')[-1]}]")
        out.append("")
        out.append("symbols:")
        syms = []
        for n, (o, s) in self.defs.items():
            sec = o.sections[s.shndx]
            if sec.live:
                syms.append((sec.addr + s.value, n))
        for n, (a, sz) in self.extra.items():
            syms.append((a, f"{n} ({sz} B)"))
        for a, n in sorted(syms):
            out.append(f"  0x{a:04x} {n}")
        return "\n".join(out) + "\n"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("objs", nargs="+")
    ap.add_argument("-T", "--layout", required=True)
    ap.add_argument("-o", "--output", required=True)
    ap.add_argument("-m", "--map")
    ap.add_argument("--no-gc-sections", action="store_true")
    a = ap.parse_args(argv)
    try:
        lk = Linker([read_elf(p) for p in a.objs], read_layout(a.layout),
                    gc=not a.no_gc_sections)
        img = lk.link()
    except LinkError as e:
        print(f"i8008_link: error: {e}", file=sys.stderr)
        return 1
    safe_out(a.output).write_bytes(img)
    if a.map:
        safe_out(a.map).write_text(lk.map_text())
    print(f"i8008_link: {a.output}: {len(img)} B at 0x{lk.lay.code[0]:04x}, "
          f"code {lk.code_end - lk.lay.code[0]} B, RAM to 0x{lk.frames_end:04x}, "
          f"depth {lk.worst}/{lk.lay.depth}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
