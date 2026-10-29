#!/usr/bin/env python3
"""Independent checks of the images built by build_profile.sh (issue #58).

    check_images.py <build dir> [<repo root>]

Nothing here uses the linker's or the compiler's code. For every image in images.json:

1. Decoders. Every executable range of the linked ELF is decoded by GNU binutils objdump (-m i8086),
   by Capstone (16-bit x86) and by llvm-objdump (this project's disassembler, itself checked against
   a physical-chip corpus). The three must find the same instruction boundaries, and every mnemonic
   must belong to the 8086/8088 instruction set (no 80186+ instruction, no 32-bit operand, no x87).
2. Relocations. The relocation entries of the input objects are read with a small ELF reader below
   and applied again, using the map file for where each input section went; the result must equal the
   bytes of the linked image (R_8, R_16, R_PC8, R_PC16, R_SEG16, segments included).
3. Placement and entry. Every loaded range lies in the format's window of the memory map and no two
   overlap; the entry point is the one expected; the image file is the ELF's memory (boot sector
   signature, .COM bytes, MZ header with its relocation table and SP).

Exit status 0 only if every check of every image passes.
"""
import json
import os
import re
import struct
import subprocess
import sys
import tempfile

import capstone

BUILD = os.path.abspath(sys.argv[1])
ROOT = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else os.path.abspath(os.path.join(BUILD, '../../../..'))
LLVM_OBJDUMP = os.path.join(ROOT, 'llvm-i8086/install/bin/llvm-objdump')

# The 8086/8088 mnemonics of Intel's 1979 manual (as Capstone and GNU spell them).
I8086 = set('''aaa aad aam aas adc add and call cbw clc cld cli cmc cmp cmpsb cmpsw cwd daa das dec div hlt idiv imul in inc
int int3 into iret ja jae jb jbe jcxz je jg jge jl jle jmp jne jno jnp jns jo jp js lahf lds lea les lock lodsb lodsw
loop loope loopne mov movsb movsw mul neg nop not or out pop popf push pushf rcl rcr ret retf rol ror sahf sal sar sbb
scasb scasw shl shr stc std sti stosb stosw sub test wait xchg xlatb xor rep repe repne repz repnz jnb jnbe jnl jnle jna
jnae jnc jc jz jnz jpe jpo je ljmp lcall lret xlat salc stos lods movs cmps scas'''.split())
PREFIXES = {'rep', 'repe', 'repne', 'repz', 'repnz', 'lock'}


# --- a small ELF32 reader --------------------------------------------------------------------------
class Elf:
    def __init__(self, path):
        d = self.d = open(path, 'rb').read()
        assert d[:4] == b'\x7fELF' and d[4] == 1 and d[5] == 1, f'{path}: not a little-endian ELF32'
        (self.type, self.machine, _, self.entry, self.phoff, self.shoff, _, _, self.phentsize, self.phnum,
         self.shentsize, self.shnum, self.shstrndx) = struct.unpack_from('<HHIIIIIHHHHHH', d, 16)
        assert self.machine == 0x8086, f'{path}: e_machine {self.machine:#x}'
        self.sections = []
        for i in range(self.shnum):
            n, t, f, a, o, s, l, inf, al, es = struct.unpack_from('<IIIIIIIIII', d, self.shoff + i * self.shentsize)
            self.sections.append(dict(name_off=n, type=t, flags=f, addr=a, off=o, size=s, link=l, info=inf, entsize=es))
        shstr = self.sections[self.shstrndx]
        for s in self.sections:
            s['name'] = self.cstr(shstr['off'] + s['name_off'])
        self.symbols = []
        for s in self.sections:
            if s['type'] == 2:  # SHT_SYMTAB
                strtab = self.sections[s['link']]
                for k in range(s['size'] // 16):
                    n, v, sz, info, other, shndx = struct.unpack_from('<IIIBBH', d, s['off'] + 16 * k)
                    self.symbols.append(dict(name=self.cstr(strtab['off'] + n), value=v, size=sz, bind=info >> 4,
                                             kind=info & 15, shndx=shndx))
        self.relocs = []  # (section index it applies to, offset, symbol index, type, addend)
        for s in self.sections:
            if s['type'] == 4:  # SHT_RELA
                for k in range(s['size'] // 12):
                    off, info, add = struct.unpack_from('<IIi', d, s['off'] + 12 * k)
                    self.relocs.append((s['info'], off, info >> 8, info & 255, add))
        self.phdrs = []
        for i in range(self.phnum):
            t, o, va, pa, fs, ms, fl, al = struct.unpack_from('<IIIIIIII', d, self.phoff + i * self.phentsize)
            self.phdrs.append(dict(type=t, off=o, vaddr=va, filesz=fs, memsz=ms, flags=fl))

    def cstr(self, off):
        return self.d[off:self.d.index(b'\0', off)].decode()


def parse_map(path):
    """-> ({(object path, input section name): (linear address, size, output section, region)},
           {region: start of its first output section}, {linker-defined symbol: value})"""
    placed, bases, syms = {}, {}, {}
    out = region = None
    part = ''
    for line in open(path):
        if line.startswith('Output sections'):
            part = 'out'
        elif line.startswith('Symbols'):
            part = 'sym'
        m = re.match(r'^  (\S+)\s+0x([0-9a-f]+)\s+\d+ bytes\s+> (\S+)', line)
        if m and part == 'out':
            out, region = m.group(1), m.group(3)
            bases.setdefault(region, int(m.group(2), 16))
            continue
        m = re.match(r'^    0x([0-9a-f]+)\s+(\d+)\s+(\S+) \((.*)\)$', line)
        if m and out and part == 'out':
            placed[(m.group(4), m.group(3))] = (int(m.group(1), 16), int(m.group(2)), out, region)
        m = re.match(r'^  0x([0-9a-f]+) (\S+)$', line)
        if m and part == 'sym':
            syms[m.group(2)] = int(m.group(1), 16)
    return placed, bases, syms


# --- decoders --------------------------------------------------------------------------------------
def merge_prefixes(insns):
    """A prefix and the instruction it prefixes are one instruction: [(addr, length, mnemonic)]."""
    out = []
    carry = None
    for addr, ln, mn in insns:
        if carry:
            addr, ln, mn = carry[0], carry[1] + ln, mn
            carry = None
        if mn in PREFIXES and mn != 'lock':
            carry = (addr, ln, mn)
            continue
        out.append((addr, ln, mn))
    if carry:
        out.append(carry)
    return out


def binutils(code, base):
    with tempfile.NamedTemporaryFile(suffix='.bin', delete=False) as f:
        f.write(code)
        name = f.name
    try:
        text = subprocess.run(['objdump', '-D', '-b', 'binary', '-m', 'i8086', '-M', 'intel', f'--adjust-vma={base}', name],
                              capture_output=True, text=True, check=True).stdout
    finally:
        os.unlink(name)
    insns = []
    for line in text.splitlines():
        m = re.match(r'^\s*([0-9a-f]+):\t((?:[0-9a-f]{2} ?)+)\s*(?:\t(\S.*))?$', line)
        if not m:
            continue
        addr, hexb, rest = int(m.group(1), 16), m.group(2).split(), m.group(3)
        if rest is None:  # continuation line of a long instruction
            a, ln, mn = insns[-1]
            insns[-1] = (a, ln + len(hexb), mn)
            continue
        words = rest.split()
        # binutils prints a prefix and its instruction as one line: "rep stos ..."
        insns.append((addr, len(hexb), words[1] if words[0] in PREFIXES and len(words) > 1 else words[0]))
    return merge_prefixes(insns)


def capstone_decode(code, base):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_16)
    def name(i):
        mn = i.mnemonic.split()[-1]          # Capstone writes "rep stosb" as one mnemonic
        # opcodes 98h and 99h are cbw and cwd in 16-bit code; Capstone uses the 32-bit names
        return {'cwde': 'cbw', 'cdq': 'cwd'}.get(mn, mn) if i.size == 1 else mn

    insns = [(i.address, i.size, name(i)) for i in md.disasm(code, base)]
    return merge_prefixes(insns)


def llvm_decode(elf_path, section_name, base, size):
    text = subprocess.run([LLVM_OBJDUMP, '-d', '--section=' + section_name, elf_path], capture_output=True,
                          text=True, check=True).stdout
    insns = []
    for line in text.splitlines():
        m = re.match(r'^\s*([0-9a-f]+):\s+((?:[0-9a-f]{2} )+)\s*(.*)$', line)
        if not m:
            continue
        addr, hexb, rest = int(m.group(1), 16), m.group(2).split(), m.group(3)
        if not rest.strip():
            a, ln, mn = insns[-1]
            insns[-1] = (a, ln + len(hexb), mn)
            continue
        insns.append((addr, len(hexb), rest.split()[0]))
    return merge_prefixes([i for i in insns if base <= i[0] < base + size])


# --- the checks ------------------------------------------------------------------------------------
class Report:
    def __init__(self):
        self.lines = []
        self.failed = 0

    def ok(self, msg):
        self.lines.append(f'  ok    {msg}')

    def bad(self, msg):
        self.lines.append(f'  FAIL  {msg}')
        self.failed += 1


def memory_of(elf):
    """linear address -> byte for the loaded part of the ELF; also the list of (start, end, loaded)."""
    mem, ranges = {}, []
    for p in elf.phdrs:
        if p['type'] != 1 or p['memsz'] == 0:
            continue
        ranges.append((p['vaddr'], p['vaddr'] + p['memsz'], p['filesz'] > 0))
        data = elf.d[p['off']:p['off'] + p['filesz']]
        for i, b in enumerate(data):
            mem[p['vaddr'] + i] = b
    return mem, ranges


def check_decoders(img, elf, elf_path, objs, placed, mem, rep):
    """Decode every executable input section (found from the objects' section flags and the map)."""
    seen = set()
    mnemonics = set()
    total = 0
    for (path, secname), (addr, size, out, _region) in sorted(placed.items(), key=lambda kv: kv[1][0]):
        osec = next((x for x in objs[path].sections if x['name'] == secname), None)
        if osec is None or not (osec['flags'] & 4) or size == 0:
            continue
        if any(addr + i not in mem for i in range(size)):
            rep.bad(f'{os.path.basename(path)} {secname}: the map puts it at {addr:#x} but the ELF has no bytes there')
            continue
        code = bytes(mem[addr + i] for i in range(size))
        a = binutils(code, addr)
        b = capstone_decode(code, addr)
        c = llvm_decode(elf_path, out, addr, size)
        lens = [[(x[0], x[1]) for x in v] for v in (a, b, c)]
        where = f'{os.path.basename(path)} {secname}'
        if not (lens[0] == lens[1] == lens[2]):
            first = next((i for i in range(min(map(len, lens))) if not (lens[0][i] == lens[1][i] == lens[2][i])),
                         min(map(len, lens)))
            rep.bad(f'{where}: the decoders disagree at instruction {first}: binutils {lens[0][first:first+1]} '
                    f'capstone {lens[1][first:first+1]} llvm {lens[2][first:first+1]} '
                    f'(counts {len(a)}/{len(b)}/{len(c)})')
            continue
        if sum(x[1] for x in a) != size:
            rep.bad(f'{where}: decoded {sum(x[1] for x in a)} of {size} bytes')
            continue
        total += len(a)
        mnemonics |= {m.lower() for _, _, m in a} | {m.lower() for _, _, m in b}
        seen.add((path, secname))
    bad = sorted(m for m in mnemonics if m not in I8086)
    if bad:
        rep.bad(f'mnemonics outside the 8086 instruction set: {bad}')
    elif total:
        rep.ok(f'{total} instructions in {len(seen)} code sections: binutils, Capstone and llvm-objdump agree on every '
               f'boundary; {len(mnemonics)} distinct mnemonics, all 8086')


def apply_relocations(img, objs, placed, bases, linker_syms, mem, seg_regions, rep):
    """Re-apply every relocation of every object that is in the map and compare with the image."""
    # global definitions: name -> (object, section name, value, weak)
    defs = {}
    for path, elf in objs.items():
        for sym in elf.symbols[1:]:
            if sym['bind'] in (1, 2) and sym['shndx'] not in (0, 0xFFF1):
                weak = sym['bind'] == 2
                old = defs.get(sym['name'])
                if old is None or (old[3] and not weak):
                    defs[sym['name']] = (path, elf.sections[sym['shndx']]['name'], sym['value'], weak)

    def seg_of(entry):
        # a region with a segment of its own starts at a paragraph, with offset 0
        _, _, _, region = entry
        return bases[region] >> 4 if region in seg_regions else 0

    n = checked = 0
    for path, elf in objs.items():
        for sec_idx, off, symi, typ, add in elf.relocs:
            sec = elf.sections[sec_idx]
            # the relocated section is ".rela.X" -> sh_info = index of X
            key = (path, sec['name'])
            if key not in placed:
                continue  # collected by the linker
            place = placed[key]
            sym = elf.symbols[symi]
            if sym['shndx'] == 0 and sym['name'] in linker_syms and sym['name'] not in defs:
                s_lin, target = linker_syms[sym['name']], (0, 0, '', '')   # defined by the linker script
            elif sym['shndx'] == 0:  # undefined here
                if sym['name'] not in defs:
                    if sym['bind'] == 2:
                        continue
                    rep.bad(f'{path}: undefined symbol {sym["name"]}')
                    continue
                dpath, dsec, dval, _ = defs[sym['name']]
                target = placed.get((dpath, dsec))
                if target is None:
                    rep.bad(f'{sym["name"]} is defined in a collected section')
                    continue
                s_lin = target[0] + dval
            else:
                tsec = elf.sections[sym['shndx']]
                target = placed.get((path, tsec['name']))
                if target is None:
                    rep.bad(f'{path}: relocation against collected section {tsec["name"]}')
                    continue
                s_lin = target[0] + (0 if sym['kind'] == 3 else sym['value'])
            tseg = seg_of(target)
            pseg = seg_of(place)
            s_off = s_lin - 16 * tseg
            p_lin = place[0] + off
            p_off = p_lin - 16 * pseg
            if typ == 1:
                want, width = (s_off + add) & 0xFF, 1
            elif typ == 2:
                want, width = (s_off + add) & 0xFFFF, 2
            elif typ in (3, 4):
                if tseg != pseg:
                    rep.bad(f'{path}: a PC-relative relocation across segments at {p_lin:#x}')
                    continue
                want, width = ((s_off + add - p_off) & (0xFF if typ == 3 else 0xFFFF)), (1 if typ == 3 else 2)
            elif typ == 5:
                want, width = tseg & 0xFFFF, 2
            else:
                rep.bad(f'{path}: unknown relocation type {typ}')
                continue
            got = int.from_bytes(bytes(mem.get(p_lin + i, 0xEE) for i in range(width)), 'little')
            n += 1
            if got != want:
                rep.bad(f'{os.path.basename(path)} {sec["name"]}+{off:#x} type {typ} -> {sym["name"] or tsec["name"]}: '
                        f'image has {got:#x}, expected {want:#x}')
            else:
                checked += 1
    if n and checked == n:
        rep.ok(f'{n} relocations re-applied from the object files equal the linked bytes')
    elif n == 0:
        rep.bad('no relocations were checked')


def check_placement(img, elf, mem, ranges, rep):
    lo, hi = img['window']
    for start, end, _ in ranges:
        if start < lo or end > hi:
            rep.bad(f'{start:#x}..{end:#x} is outside the window {lo:#x}..{hi:#x}')
    ordered = sorted(ranges)
    for a, b in zip(ordered, ordered[1:]):
        if a[1] > b[0]:
            rep.bad(f'ranges {a[0]:#x}..{a[1]:#x} and {b[0]:#x}..{b[1]:#x} overlap')
    if not rep.failed:
        rep.ok(f'{len(ranges)} loaded ranges inside {lo:#x}..{hi:#x}, no overlap')
    if img['entry'] is not None:
        if elf.entry != img['entry']:
            rep.bad(f'entry {elf.entry:#x}, expected {img["entry"]:#x}')
        else:
            rep.ok(f'entry point {elf.entry:#x}')


def check_file(img, elf, mem, ranges, rep):
    data = open(os.path.join(BUILD, img['image']), 'rb').read()

    def elf_bytes(start, n):
        return bytes(mem.get(start + i, 0) for i in range(n))

    if img['kind'] == 'floppy':
        if len(data) != 163840:
            rep.bad(f'floppy image is {len(data)} bytes, not 160 KiB')
        if data[510:512] != b'\x55\xaa':
            rep.bad('no 55AAh boot signature')
        if data[:510] != elf_bytes(0x7C00, 510):
            rep.bad('the boot sector is not the ELF memory at 7C00h')
        top = max(e for _, e, loaded in ranges if loaded)
        if data[512:512 + (top - 0x7E00)] != elf_bytes(0x7E00, top - 0x7E00):
            rep.bad('stage 2 is not the ELF memory at 7E00h')
        if not rep.failed:
            rep.ok('160 KiB, boot signature 55AAh, boot sector and stage 2 equal the ELF memory')
    elif img['kind'] == 'com':
        top = max(e for _, e, loaded in ranges if loaded)
        if data != elf_bytes(0x100, top - 0x100):
            rep.bad('the .COM is not the ELF memory from 0100h')
        else:
            rep.ok(f'.COM of {len(data)} bytes equals the ELF memory from 0100h (loads at CS:0100h)')
    elif img['kind'] == 'exe':
        w = lambda o: struct.unpack_from('<H', data, o)[0]
        assert data[:2] == b'MZ'
        hdr = w(8) * 16
        nrel, tab = w(6), w(24)
        image = data[hdr:]
        pages = w(4)
        if (pages - 1) * 512 + (w(2) or 512) != len(data):
            rep.bad('MZ size fields do not match the file size')
        if w(12) != 0xFFFF or hdr != 512:
            rep.bad('MZ maximum allocation or header size unexpected')
        relocs = set()
        for i in range(nrel):
            o, s = w(tab + 4 * i), w(tab + 4 * i + 2)
            relocs.add(s * 16 + o)
        # the image is the ELF memory from 0100h with every relocated word reduced by 10h
        top = max(e for _, e, loaded in ranges if loaded)
        want = bytearray(elf_bytes(0x100, top - 0x100))
        for r in relocs:
            v = (struct.unpack_from('<H', want, r)[0] - 0x10) & 0xFFFF
            struct.pack_into('<H', want, r, v)
        if image != bytes(want):
            rep.bad('the .EXE body is not the ELF memory with the relocated words reduced by 10h')
        if w(22) != 0xFFF0 or w(14) != 0xFFF0 or w(20) != elf.entry:
            rep.bad(f'MZ CS:IP/SS = {w(22):#x}:{w(20):#x}/{w(14):#x}, expected FFF0:{elf.entry:#x}/FFF0')
        if not rep.failed:
            rep.ok(f'MZ header, {nrel} segment relocations, CS:IP = FFF0h:{w(20):#x}, SP {w(16):#x}, body equals the ELF memory')


def main():
    images = json.load(open(os.path.join(BUILD, 'images.json')))
    rtdir = os.path.join(ROOT, 'llvm-i8086/rt/obj')
    failed = 0
    elf_cache = {}
    for img in images:
        rep = Report()
        elf_path = os.path.join(BUILD, img['elf'])
        elf = elf_cache.setdefault(elf_path, Elf(elf_path))
        mem, ranges = memory_of(elf)
        placed_raw, bases, linker_syms = parse_map(os.path.join(BUILD, img['map']))
        objs = {}
        for o in img['objects']:
            paths = sorted(os.path.join(rtdir, f) for f in os.listdir(rtdir) if f.endswith('.o')) if o == '@rt' \
                else [os.path.join(BUILD, o)]
            for p in paths:
                objs[p] = Elf(p)
        # map paths are as given to the linker (relative to the repository root, or absolute)
        placed = {}
        for (obj, sec), v in placed_raw.items():
            if 'toolchain/build/' in obj:      # an object of this build, wherever the build directory is now
                ap = os.path.join(BUILD, obj.split('toolchain/build/', 1)[1])
            else:
                ap = obj if os.path.isabs(obj) else os.path.join(ROOT, obj)
            placed[(os.path.normpath(ap), sec)] = v
        objs = {os.path.normpath(p): e for p, e in objs.items()}
        check_decoders(img, elf, elf_path, objs, placed, mem, rep)
        apply_relocations(img, objs, placed, bases, linker_syms, mem, set(img.get('segment_regions', [])), rep)
        check_placement(img, elf, mem, ranges, rep)
        check_file(img, elf, mem, ranges, rep)
        print(f'{img["name"]}: {"PASS" if not rep.failed else "FAIL"}')
        print('\n'.join(rep.lines))
        failed += rep.failed
    print('ALL PASS' if not failed else f'{failed} CHECKS FAILED')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
