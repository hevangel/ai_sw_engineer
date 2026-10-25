#!/usr/bin/env node
// Regenerates encodings.s from the table below: each row is an instruction and
// the bytes the Intel 8086 Family User's Manual's opcode table gives for it.
// The table is written out by hand from that manual (not produced by the
// assembler); vector_check.py then checks the whole backend against the
// physical-chip corpus as a second, independent source.
const rows = `
add bl, cl | 00 cb
add byte ptr [bx], cl | 00 0f
add cl, byte ptr [bx] | 02 0f
add bx, cx | 01 cb
add word ptr [bx+si], cx | 01 08
add cx, word ptr [bx+si] | 03 08
add al, 0x12 | 04 12
add ax, 0x1234 | 05 34 12
add bl, 5 | 80 c3 05
add bx, 5 | 83 c3 05
add bx, -1 | 83 c3 ff
add bx, 0x1234 | 81 c3 34 12
add byte ptr [bx], 5 | 80 07 05
add word ptr [bx], 5 | 83 07 05
add word ptr [bx], 0x1234 | 81 07 34 12
or al, 1 | 0c 01
adc al, 1 | 14 01
sbb al, 1 | 1c 01
and al, 1 | 24 01
sub al, 1 | 2c 01
xor al, 1 | 34 01
cmp al, 1 | 3c 01
or bx, cx | 09 cb
adc bx, cx | 11 cb
sbb bx, cx | 19 cb
and bx, cx | 21 cb
sub bx, cx | 29 cb
xor bx, cx | 31 cb
cmp bx, cx | 39 cb
cmp bx, -1 | 83 fb ff
sub bx, 1000 | 81 eb e8 03
and ax, 0xff00 | 25 00 ff
test al, 1 | a8 01
test ax, 0x100 | a9 00 01
test bl, cl | 84 cb
test bx, cx | 85 cb
test byte ptr [bx], 1 | f6 07 01
test cx, 1 | f7 c1 01 00
not al | f6 d0
neg bl | f6 db
mul cl | f6 e1
imul dl | f6 ea
div bl | f6 f3
idiv bh | f6 ff
not word ptr [bx] | f7 17
mul bx | f7 e3
neg word ptr [si] | f7 1c
inc ax | 40
inc di | 47
dec ax | 48
dec di | 4f
inc al | fe c0
dec bl | fe cb
inc byte ptr [bx] | fe 07
dec word ptr [bx] | ff 0f
rol al, 1 | d0 c0
ror ax, 1 | d1 c8
rcl al, 1 | d0 d0
rcr ax, cl | d3 d8
shl al, 1 | d0 e0
sal ax, 1 | d1 e0
shr bl, cl | d2 eb
sar bx, cl | d3 fb
shl byte ptr [bx], 1 | d0 27
mov al, bl | 88 d8
mov ax, bx | 89 d8
mov byte ptr [bx], al | 88 07
mov word ptr [bx], ax | 89 07
mov al, byte ptr [bx] | 8a 07
mov ax, word ptr [bx] | 8b 07
mov al, 0x12 | b0 12
mov bl, 0x12 | b3 12
mov ah, 1 | b4 01
mov ax, 0x1234 | b8 34 12
mov di, 0x1234 | bf 34 12
mov byte ptr [bx], 7 | c6 07 07
mov word ptr [bx], 0x1234 | c7 07 34 12
mov al, byte ptr [0x1234] | a0 34 12
mov ax, word ptr [0x1234] | a1 34 12
mov byte ptr [0x1234], al | a2 34 12
mov word ptr [0x1234], ax | a3 34 12
mov bl, byte ptr [0x1234] | 8a 1e 34 12
mov ax, es | 8c c0
mov word ptr [bx], ds | 8c 1f
mov es, ax | 8e c0
mov ds, word ptr [bx] | 8e 1f
mov ss, bx | 8e d3
xchg ax, bx | 93
xchg bx, ax | 87 c3
xchg bl, cl | 86 cb
xchg byte ptr [bx], cl | 86 0f
xchg cl, byte ptr [bx] | 86 0f
lea ax, [bx+si+4] | 8d 40 04
les bx, dword ptr [si] | c4 1c
lds si, dword ptr [bx] | c5 37
mov al, byte ptr [bx+si] | 8a 00
mov al, byte ptr [bx+di] | 8a 01
mov al, byte ptr [bp+si] | 8a 02
mov al, byte ptr [bp+di] | 8a 03
mov al, byte ptr [si] | 8a 04
mov al, byte ptr [di] | 8a 05
mov al, byte ptr [bp] | 8a 46 00
mov al, byte ptr [bx] | 8a 07
mov al, byte ptr [bp+4] | 8a 46 04
mov al, byte ptr [bx+0x1234] | 8a 87 34 12
mov al, byte ptr [bx-1] | 8a 47 ff
mov al, byte ptr [si+0x80] | 8a 84 80 00
mov al, byte ptr [bx+si-128] | 8a 40 80
mov al, byte ptr [bx+si+127] | 8a 40 7f
mov al, byte ptr es:[bx] | 26 8a 07
mov al, byte ptr cs:[bx] | 2e 8a 07
mov al, byte ptr ss:[bx+4] | 36 8a 47 04
mov al, byte ptr ds:[bx] | 3e 8a 07
mov byte ptr es:[0x10], al | 26 a2 10 00
push ax | 50
push di | 57
pop ax | 58
pop di | 5f
push es | 06
push cs | 0e
push ss | 16
push ds | 1e
pop es | 07
pop ss | 17
pop ds | 1f
push word ptr [bx] | ff 37
pop word ptr [bx] | 8f 07
pushf | 9c
popf | 9d
sahf | 9e
lahf | 9f
ret | c3
ret 4 | c2 04 00
retf | cb
retf 2 | ca 02 00
int3 | cc
int 0x21 | cd 21
into | ce
iret | cf
hlt | f4
nop | 90
wait | 9b
xlat | d7
cbw | 98
cwd | 99
clc | f8
stc | f9
cli | fa
sti | fb
cld | fc
std | fd
cmc | f5
daa | 27
das | 2f
aaa | 37
aas | 3f
aam | d4 0a
aad | d5 0a
aam 16 | d4 10
jmp bx | ff e3
jmp word ptr [bx] | ff 27
call bx | ff d3
call word ptr [bx] | ff 17
jmp dword ptr [bx] | ff 2f
call dword ptr [bx] | ff 1f
jmp 0x1000:0x100 | ea 00 01 00 10
call 0x1000:0x100 | 9a 00 01 00 10
in al, 0x60 | e4 60
in ax, 0x60 | e5 60
out 0x61, al | e6 61
out 0x61, ax | e7 61
in al, dx | ec
in ax, dx | ed
out dx, al | ee
out dx, ax | ef
movsb | a4
movsw | a5
cmpsb | a6
cmpsw | a7
stosb | aa
stosw | ab
lodsb | ac
lodsw | ad
scasb | ae
scasw | af
rep | f3
repne | f2
lock | f0
`.trim().split('\n').map(l => l.split('|').map(s => s.trim()));

const hx = (...b) => b.map(x => x.toString(16).padStart(2, '0')).join(' ');
const alu = ['add','or','adc','sbb','and','sub','xor','cmp'];
alu.forEach((n, k) => {
  const b = k * 8, m = 0xc0 | (k << 3) | 3, mm = (k << 3) | 7;
  rows.push([n + ' bl, cl', hx(b, 0xcb)], [n + ' byte ptr [bx], cl', hx(b, 0x0f)],
    [n + ' bx, cx', hx(b + 1, 0xcb)], [n + ' word ptr [bx+si], cx', hx(b + 1, 0x08)],
    [n + ' cl, byte ptr [bx]', hx(b + 2, 0x0f)], [n + ' cx, word ptr [bx+si]', hx(b + 3, 0x08)],
    [n + ' al, 0x12', hx(b + 4, 0x12)], [n + ' ax, 0x1234', hx(b + 5, 0x34, 0x12)],
    [n + ' bl, 5', hx(0x80, m, 5)], [n + ' bx, 5', hx(0x83, m, 5)], [n + ' bx, -2', hx(0x83, m, 0xfe)],
    [n + ' bx, 0x1234', hx(0x81, m, 0x34, 0x12)], [n + ' byte ptr [bx], 5', hx(0x80, mm, 5)],
    [n + ' word ptr [bx], 5', hx(0x83, mm, 5)], [n + ' word ptr [bx], 0x1234', hx(0x81, mm, 0x34, 0x12)],
    [n + ' cl, bl', hx(b, 0xd9)], [n + ' cx, bx', hx(b + 1, 0xd9)]);
});
const shifts = {rol: 0, ror: 1, rcl: 2, rcr: 3, shl: 4, shr: 5, sar: 7};
for (const [n, d] of Object.entries(shifts)) {
  const m = 0xc0 | (d << 3) | 3, mm = (d << 3) | 7;
  rows.push([n + ' al, 1', hx(0xd0, 0xc0 | (d << 3))], [n + ' bx, 1', hx(0xd1, m)],
    [n + ' bl, cl', hx(0xd2, m)], [n + ' bx, cl', hx(0xd3, m)],
    [n + ' byte ptr [bx], 1', hx(0xd0, mm)], [n + ' word ptr [bx], 1', hx(0xd1, mm)],
    [n + ' byte ptr [bx], cl', hx(0xd2, mm)], [n + ' word ptr [bx], cl', hx(0xd3, mm)]);
}
const unary = {not: 2, neg: 3, mul: 4, imul: 5, div: 6, idiv: 7};
for (const [n, d] of Object.entries(unary)) {
  rows.push([n + ' bl', hx(0xf6, 0xc0 | (d << 3) | 3)], [n + ' bx', hx(0xf7, 0xc0 | (d << 3) | 3)],
    [n + ' byte ptr [bx]', hx(0xf6, (d << 3) | 7)], [n + ' word ptr [bx]', hx(0xf7, (d << 3) | 7)]);
}
rows.push(['xchg word ptr [bx], cx', '87 0f'], ['xchg cx, word ptr [bx]', '87 0f'],
  ['test word ptr [bx], cx', '85 0f'], ['test cx, word ptr [bx]', '85 0f'],
  ['test cl, byte ptr [bx]', '84 0f'], ['test byte ptr [bx], cl', '84 0f'],
  ['test word ptr [bx], 0x1234', 'f7 07 34 12'], ['dec byte ptr [bx]', 'fe 0f'],
  ['inc word ptr [bx]', 'ff 07'], ['mov word ptr [bx], cx', '89 0f'], ['mov cx, word ptr [bx]', '8b 0f'],
  ['mov cl, byte ptr [bx]', '8a 0f'], ['mov byte ptr [bx], cl', '88 0f'], ['mov cx, ds', '8c d9'],
  ['mov ds, cx', '8e d9'], ['push word ptr [bx]', 'ff 37'], ['out dx, ax', 'ef'], ['aad 7', 'd5 07'], ['test bl, 5', 'f6 c3 05']);
const out = [];
out.push('# RUN: llvm-mc -triple=i8086 -show-encoding %s | FileCheck %s');
out.push('# Generated by gen_encodings.js from the manual\'s opcode table; do not edit.');
out.push('');
let first = true;
for (const [asm, bytes] of rows) {
  out.push(asm);
  const enc = bytes.split(' ').map(b => '0x' + b).join(',');
  out.push(`# CHECK${first ? '' : '-NEXT'}: {{.*}}; encoding: [${enc}]`);
  first = false;
}

out.push('', '# Branch encodings carry a PC-relative fixup (the displacement is not yet known).');
const jcc = {jo:0x70,jno:0x71,jb:0x72,jc:0x72,jnae:0x72,jae:0x73,jnb:0x73,jnc:0x73,je:0x74,jz:0x74,jne:0x75,jnz:0x75,
  jbe:0x76,jna:0x76,ja:0x77,jnbe:0x77,js:0x78,jns:0x79,jp:0x7a,jpe:0x7a,jnp:0x7b,jpo:0x7b,jl:0x7c,jnge:0x7c,jge:0x7d,
  jnl:0x7d,jle:0x7e,jng:0x7e,jg:0x7f,jnle:0x7f,loop:0xe2,loope:0xe1,loopz:0xe1,loopne:0xe0,loopnz:0xe0,jcxz:0xe3,
  'jmp short':0xeb};
const labels = [];
let firstJ = true;
for (const [m, op] of Object.entries(jcc)) {
  out.push(m + ' done');
  out.push('# CHECK' + (firstJ ? '' : '-NEXT') + ': {{.*}}; encoding: [0x' + hx(op) + ',A]');
  firstJ = false;
  out.push('# CHECK-NEXT: {{.*}}fixup A - offset: 1, value: done-1, kind: fixup_pc8');
}
for (const [m, op] of [['jmp near', 0xe9], ['call', 0xe8]]) {
  out.push(m + ' done');
  out.push('# CHECK-NEXT: {{.*}}; encoding: [0x' + hx(op) + ',A,A]');
  out.push('# CHECK-NEXT: {{.*}}fixup A - offset: 1, value: done-2, kind: fixup_pc16');
}
out.push('done:');
require('fs').writeFileSync(__dirname + '/encodings.s', out.join('\n') + '\n');
console.log(rows.length + ' rows');
