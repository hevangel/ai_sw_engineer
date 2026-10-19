#!/usr/bin/env bash
# Execution-test for the per-function FRAME correctness fix (abi.md 4.4 / brief
# decision 1-2). A caller and callee that BOTH spill must not clobber each
# other's spilled values. Builds a chain where each level forces several i8
# values live across a call (so they spill), nested depth >= 4, and checks the
# final accumulated result on chips. Under the OLD fixed-0x30 shared spill
# region every function spills to the same chars, so a callee overwrites the
# caller's spill -> wrong result; the per-function frame fix makes it correct.
#
# Same-bank (flat enough) link so this isolates FRAME correctness from banking;
# also run a banked variant to prove frames work with DCL bank switching.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/build/llvm-obj/bin"; [[ -x "$B/llc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/frex.XXXXXX")}"
mkdir -p "$WORK/h/src"
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }
[[ -x "$B/llc" ]] || fail "llc missing at $B"

# bank-reloading harness (same shape as run_softcall_banked_exec).
COMMON=0x800; BANKW=0x400
COMMONI=$((COMMON)); BANKI=$((BANKW))
cat > "$WORK/h/Cargo.toml" <<EOF
[package]
name="frex"
version="0.1.0"
edition="2021"
[[bin]]
name="frex"
path="src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cat > "$WORK/h/src/main.rs" <<RS
use chips::mcs4::Board; use arbitrary_int::u4;
const COMMON: usize = $COMMONI; const BANKW: usize = $BANKI;
fn view(c:&[u8],b:&[u8],bk:usize)->Vec<u8>{let mut v=vec![0u8;0x1000];let n=c.len().min(COMMON);v[..n].copy_from_slice(&c[..n]);let o=bk*BANKW;if o<b.len(){let m=(b.len()-o).min(BANKW);v[COMMON..COMMON+m].copy_from_slice(&b[o..o+m]);}v}
fn main(){let a:Vec<String>=std::env::args().collect();let img=std::fs::read(&a[1]).unwrap();let cyc:usize=a[2].parse().unwrap();let nch:usize=a[3].parse().unwrap();
let c=&img[..COMMON.min(img.len())];let bk=if img.len()>COMMON{&img[COMMON..]}else{&[][..]};let fb=COMMON/0x100;let mut bank=0usize;let mut b=Board::new(view(c,bk,bank),2);let mut prev=b.roms[3].ports.value();
for _ in 0..cyc{b.run_cycle();let l=b.roms[3].ports.value() as usize;if l!=prev as usize{prev=l as u8;if l!=bank{bank=l;let v=view(c,bk,bank);for i in fb..16{if i<b.roms.len(){let s=i*0x100;let ch:[u8;0x100]=v[s..s+0x100].try_into().unwrap();b.roms[i]=chips::mcs4::rom4001::ROM::new(ch,u4::new(i as u8));}}}}}
let mut out=Vec::new();for ch in 0..nch{out.push(b.rams[0].registers[0].characters.read_nibble(ch as u8).value().to_string());}
println!("{}",out.join(" "));}
RS
( cd "$WORK/h" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/c.log" 2>&1 ) \
    || { tail -25 "$WORK/c.log" >&2; fail "harness build"; }
H="$WORK/ct/release/frex"

cat > "$WORK/reset.s" <<'EOF'
	.section .reset,"ax"
	.globl _start
_start:
	ldm	0
	dcl
	fim	p0, 0x0f
	src	p0
	ldm	0
	wrm
	fim	p7, 0x40
	fim	p0, %lo8(__halt)
	jms	__push_off
	fim	p0, %page(__halt)
	jms	__push_pg
	jun	entry
	.globl __halt
__halt:	jun __halt
EOF

# Each gN keeps FOUR values live across the recursive call so the register
# allocator must spill them; after the call it combines them. If a callee
# clobbers the caller's spill slots the sum is wrong. Expected: gN returns
# a known closed-form; entry stores g1(1) low/hi to chars 0,1.
#   leaf g5(x) = x + 100
#   gN(x) = let a=x+1,b=x+2,c=x+3,d=x+4 in a+b+c+d + g(N+1)(x+1)
# These four locals are live across the call -> forced spills. Kept small so the
# accumulated result never overflows i8 (a wrap would be correct too, but a
# clean < 256 total makes the oracle obvious). Leaf returns its arg unchanged;
# each level adds its four live locals (a+b+c+d) plus the callee result.
cat > "$WORK/t.ll" <<'EOF'
target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"
target triple = "i4004"
define i8 @g5(i8 %x){ ret i8 %x }
define i8 @g4(i8 %x){ %a=add i8 %x,1 %b=add i8 %x,1 %c=add i8 %x,1 %d=add i8 %x,1
  %n=call i8 @g5(i8 %a) %s1=add i8 %a,%b %s2=add i8 %c,%d %s3=add i8 %s1,%s2 %r=add i8 %s3,%n ret i8 %r }
define i8 @g3(i8 %x){ %a=add i8 %x,1 %b=add i8 %x,1 %c=add i8 %x,1 %d=add i8 %x,1
  %n=call i8 @g4(i8 %a) %s1=add i8 %a,%b %s2=add i8 %c,%d %s3=add i8 %s1,%s2 %r=add i8 %s3,%n ret i8 %r }
define i8 @g2(i8 %x){ %a=add i8 %x,1 %b=add i8 %x,1 %c=add i8 %x,1 %d=add i8 %x,1
  %n=call i8 @g3(i8 %a) %s1=add i8 %a,%b %s2=add i8 %c,%d %s3=add i8 %s1,%s2 %r=add i8 %s3,%n ret i8 %r }
define i8 @g1(i8 %x){ %a=add i8 %x,1 %b=add i8 %x,1 %c=add i8 %x,1 %d=add i8 %x,1
  %n=call i8 @g2(i8 %a) %s1=add i8 %a,%b %s2=add i8 %c,%d %s3=add i8 %s1,%s2 %r=add i8 %s3,%n ret i8 %r }
define void @entry(){
  %r=call i8 @g1(i8 0)
  store volatile i8 %r, ptr inttoptr (i16 0 to ptr)
  ret void
}
EOF

# host oracle for the depth-4 nest (nested depth 5 incl. leaf): each level has
# a=b=c=d=x+1, s3=4*(x+1), plus the callee result; leaf is identity.
status "building host oracle"
cat > "$WORK/oracle.py" <<'PY'
def g5(x): return x & 0xff
def step(x, n):
    a = (x + 1) & 0xff
    s3 = (4 * a) & 0xff
    return (s3 + n) & 0xff
def g4(x): a=(x+1)&0xff; return step(x, g5(a))
def g3(x): a=(x+1)&0xff; return step(x, g4(a))
def g2(x): a=(x+1)&0xff; return step(x, g3(a))
def g1(x): a=(x+1)&0xff; return step(x, g2(a))
r = g1(0)
print(r & 0xf, (r >> 4) & 0xf)
PY
want=$(python3 "$WORK/oracle.py")
status "host oracle g1(0) -> chars [$want]"

build_run () {  # $1 = extra pack args ; $2 = label
  "$B/llc" -mtriple=i4004 -function-sections -filetype=obj "$WORK/t.ll" -o "$WORK/t.o" 2>"$WORK/llc.log" || { cat "$WORK/llc.log" >&2; fail "llc"; }
  "$B/llvm-mc" -triple i4004 --filetype=obj "$WORK/reset.s" -o "$WORK/reset.o" 2>/dev/null
  objs=("$WORK/reset.o")
  for s in arith arith16 bcd logic logic16 shift shift16 mem stack regops; do
    "$B/llvm-mc" -triple i4004 --filetype=obj "$RT/$s.s" -o "$WORK/$s.o" 2>/dev/null && objs+=("$WORK/$s.o")
  done
  objs+=("$WORK/t.o")
  python3 "$PACK" "${objs[@]}" -o "$WORK/img.bin" -m "$WORK/img.map" $1 --softcall 2>"$WORK/pk.log" || { cat "$WORK/pk.log" >&2; fail "link ($2)"; }
  got=$("$H" "$WORK/img.bin" 80000 2)
  if [[ "$got" == "$want" ]]; then
    echo "FRAME EXEC ($2): PASS (g1(0) chars [$got])" >&2
  else
    echo "FRAME EXEC ($2): FAIL got [$got] want [$want]" >&2
    return 1
  fi
}

rc=0
status "=== large bank (functions co-located; isolates frame correctness) ==="
build_run "--banks 16 --common-bytes 0x800 --bank-bytes 0x800" "same-bank" || rc=1
status "=== small bank (every call cross-bank; frames + DCL + far-calls) ==="
build_run "--banks 16 --common-bytes $COMMON --bank-bytes $BANKW" "banked" || rc=1
exit $rc
