#!/usr/bin/env python3
"""Negative controls for check_images.py: damage a copy of the build in five ways, each of which the
checker must report. A checker that never fails proves nothing.

    mutants.py <build dir> [<repo root>]
"""
import os
import shutil
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import check_images as ci  # noqa: E402

BUILD = os.path.abspath(sys.argv[1])
ROOT = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else ci.ROOT


def run(tmp):
    r = subprocess.run([sys.executable, os.path.join(HERE, 'check_images.py'), tmp, ROOT], capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def fresh():
    tmp = tempfile.mkdtemp(prefix='mut_')
    for f in os.listdir(BUILD):
        p = os.path.join(BUILD, f)
        if os.path.isfile(p):
            shutil.copy(p, tmp)
    # objects are referenced relative to the build dir
    for d in ('obj', 'rt'):
        shutil.copytree(os.path.join(BUILD, d), os.path.join(tmp, d))
    return tmp


def patch(path, offset, data):
    b = bytearray(open(path, 'rb').read())
    b[offset:offset + len(data)] = data
    open(path, 'wb').write(b)


def file_offset_of(elf, addr):
    for p in elf.phdrs:
        if p['type'] == 1 and p['vaddr'] <= addr < p['vaddr'] + p['filesz']:
            return p['off'] + addr - p['vaddr']
    raise KeyError(hex(addr))


results = []


def case(name, expect, mutate):
    tmp = fresh()
    try:
        mutate(tmp)
        rc, out = run(tmp)
        ok = rc != 0 and any(e in out for e in ((expect,) if isinstance(expect, str) else expect))
        results.append(ok)
        print(f'{"ok  " if ok else "FAIL"} {name}: checker {"reported" if ok else "did NOT report"} {expect}')
        if not ok:
            print(out[-600:])
    finally:
        shutil.rmtree(tmp)


def relocated_byte(tmp):
    elf = ci.Elf(os.path.join(tmp, 'hello_rs.elf'))
    obj = ci.Elf(os.path.join(tmp, 'obj/hello_rs.o'))
    placed, _, _ = ci.parse_map(os.path.join(tmp, 'hello_rs.map'))
    for sec_idx, off, symi, typ, add in obj.relocs:
        key = next((k for k in placed if os.path.basename(k[0]) == 'hello_rs.o' and k[1] == obj.sections[sec_idx]['name']), None)
        if key and typ == 2:
            return os.path.join(tmp, 'hello_rs.elf'), file_offset_of(elf, placed[key][0] + off), elf
    raise SystemExit('no relocation found')


def m_reloc(tmp):
    path, off, _ = relocated_byte(tmp)
    patch(path, off, bytes([open(path, 'rb').read()[off] ^ 0x40]))


def m_pusha(tmp):
    elf = ci.Elf(os.path.join(tmp, 'hello_rs.elf'))
    text = next(s for s in elf.sections if s['name'] == '.text')
    patch(os.path.join(tmp, 'hello_rs.elf'), text['off'] + 3, b'\x60')   # pusha, an 80186 instruction


def m_signature(tmp):
    patch(os.path.join(tmp, 'hello_rs.img'), 510, b'\x00\x00')


def m_window(tmp):
    path = os.path.join(tmp, 'hello_rs.elf')
    elf = ci.Elf(path)
    for i, p in enumerate(elf.phdrs):
        if p['type'] == 1 and p['vaddr'] == 0x500 or p['vaddr'] == 0x7E00:
            patch(path, elf.phoff + i * elf.phentsize + 8, struct.pack('<I', 0x400))   # vaddr below 0500h
            return
    raise SystemExit('no segment to move')


def m_exe_word(tmp):
    data = bytearray(open(os.path.join(tmp, 'segx.exe'), 'rb').read())
    rel = struct.unpack_from('<HH', data, 0x1C)   # the first relocation entry: offset, segment
    at = 512 + rel[1] * 16 + rel[0]
    data[at] ^= 0x01
    open(os.path.join(tmp, 'segx.exe'), 'wb').write(data)


case('a relocated word changed', 'image has', m_reloc)
case('an 80186 instruction (pusha) in the code', ('decoders disagree', 'outside the 8086'), m_pusha)
case('the boot signature wiped', 'boot signature', m_signature)
case('a loaded range moved below 0500h', 'outside the window', m_window)
case('a segment word in the .EXE changed', 'body is not the ELF memory', m_exe_word)
print('all negative controls detected' if all(results) and len(results) == 5 else 'NEGATIVE CONTROLS FAILED')
sys.exit(0 if all(results) and len(results) == 5 else 1)
