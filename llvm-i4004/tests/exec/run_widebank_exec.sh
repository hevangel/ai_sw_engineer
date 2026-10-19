#!/usr/bin/env bash
# Execution-test the 8-BIT ROM bank latch (brief step 3): cross-bank calls whose
# callees sit in banks > 15 (reachable only with the high latch nibble on 4001
# chip-4's port). A long chain of padded functions scatters ~1 per bank with
# --bank-bytes 0x100, so the deep calls land in banks 17+, forcing the forward
# token to carry a non-zero bank.hi and __soft_ret to drive both latch ports.
# Also checks an i16 return through a > bank-16 callee. The bank-reloading chips
# harness reads bank = (port4<<4)|port3 and reloads the banked ROM on a change.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/build/llvm-obj/bin"; [[ -x "$B/llc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/wb.XXXXXX")}"
mkdir -p "$WORK/h/src"
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }
[[ -x "$B/llc" ]] || fail "llc missing at $B"

NBANKS=48
COMMON=0xC00; BANKW=0x400
COMMONI=$((COMMON)); BANKI=$((BANKW))

cat > "$WORK/h/Cargo.toml" <<EOF
[package]
name="wb"
version="0.1.0"
edition="2021"
[[bin]]
name="wb"
path="src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cat > "$WORK/h/src/main.rs" <<RS
// >16-bank harness: 8-bit latch = (roms[4]<<4)|roms[3]. image = common(COMMON)
// then per-bank windows of BANKW. Reload banked chips on any latch change.
use chips::mcs4::Board; use arbitrary_int::u4;
const COMMON: usize = $COMMONI; const BANKW: usize = $BANKI;
fn view(c:&[u8],b:&[u8],bk:usize)->Vec<u8>{let mut v=vec![0u8;0x1000];let n=c.len().min(COMMON);v[..n].copy_from_slice(&c[..n]);let o=bk*BANKW;if o<b.len(){let m=(b.len()-o).min(BANKW).min(0x1000-COMMON);v[COMMON..COMMON+m].copy_from_slice(&b[o..o+m]);}v}
fn main(){let a:Vec<String>=std::env::args().collect();let img=std::fs::read(&a[1]).unwrap();let cyc:usize=a[2].parse().unwrap();let nch:usize=a[3].parse().unwrap();
let c=&img[..COMMON.min(img.len())];let bk=if img.len()>COMMON{&img[COMMON..]}else{&[][..]};let fb=COMMON/0x100;let mut bank=0usize;let mut b=Board::new(view(c,bk,bank),2);let mut prev=0u8;
for _ in 0..cyc{b.run_cycle();let lo=b.roms[3].ports.value();if lo!=prev{prev=lo;let l=((((b.roms[4].ports.value() as usize)&0xF)<<4)|((lo as usize)&0xF)) as usize;if l!=bank{bank=l;let v=view(c,bk,bank);for i in fb..16{if i<b.roms.len(){let s=i*0x100;let ch:[u8;0x100]=v[s..s+0x100].try_into().unwrap();b.roms[i]=chips::mcs4::rom4001::ROM::new(ch,u4::new(i as u8));}}}}}
let mut out=Vec::new();for ch in 0..nch{out.push(b.rams[0].registers[0].characters.read_nibble(ch as u8).value().to_string());}
println!("{}",out.join(" "));}
RS
( cd "$WORK/h" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/c.log" 2>&1 ) \
    || { tail -25 "$WORK/c.log" >&2; fail "harness build"; }
H="$WORK/ct/release/wb"

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
	fim	p0, 0x0e
	src	p0
	ldm	0
	wrm
	fim	p0, 0x30
	src	p0
	ldm	0
	wrr
	fim	p0, 0x40
	src	p0
	ldm	0
	wrr
	fim	p7, 0x40
	fim	p0, %lo8(__halt)
	jms	__push_off
	fim	p0, %page(__halt)
	jms	__push_pg
	jun	entry
	.globl __halt
__halt:	jun __halt
EOF

# Targets the 8-bit bank LATCH (high nibble on 4001 chip-4). Many UNCALLED big
# ~1-bank filler functions are placed FIRST (the packer bin-packs all functions;
# the uncalled ones take the low banks), pushing the single reachable leaf and
# the i16 helper into banks > 15 -- reachable only via the high latch nibble.
# entry makes ONE cross-bank call to that high-bank leaf and one i16 call,
# keeping call depth shallow (a separate pre-existing soft-stack depth limit,
# independent of the latch width, is out of scope).
NFILL=36
bigbody() {
  local a
  for a in 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63; do
    echo "  store volatile i8 %x, ptr inttoptr (i16 ${a} to ptr)"
  done
}
gen() {
  echo 'target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"'
  echo 'target triple = "i4004"'
  # uncalled big fillers (placed first -> low banks).
  local k
  for (( k=1; k<=NFILL; k++ )); do
    echo "define i8 @fill${k}(i8 %x){"
    bigbody
    echo "  %r=add i8 %x,1 ret i8 %r"
    echo '}'
  done
  echo "define i8 @leaf(i8 %x){ %r=add i8 %x,7 ret i8 %r }"
  echo "define i16 @addw2(i16 %a,i16 %b){ %r=add i16 %a,%b ret i16 %r }"
  echo "define i16 @wide(i16 %a,i16 %b){ %r=call i16 @addw2(i16 %a,i16 %b) ret i16 %r }"
  echo 'define void @entry(){'
  echo "  %a=call i8 @leaf(i8 1) store volatile i8 %a, ptr inttoptr (i16 0 to ptr)"
  echo "  %w=call i16 @wide(i16 258,i16 772) store volatile i16 %w, ptr inttoptr (i16 4 to ptr)"
  echo '  ret void'
  echo '}'
}
gen > "$WORK/t.ll"

status "llc + assemble + banked link ($NBANKS banks, bank=$BANKW)"
timeout 120 "$B/llc" -mtriple=i4004 -function-sections -filetype=obj "$WORK/t.ll" -o "$WORK/t.o" \
    2>"$WORK/llc.log" || { cat "$WORK/llc.log" >&2; fail "llc"; }
timeout 60 "$B/llvm-mc" -triple i4004 --filetype=obj "$WORK/reset.s" -o "$WORK/reset.o" 2>/dev/null
objs=("$WORK/reset.o")
for s in arith arith16 bcd logic logic16 shift shift16 mem stack regops; do
    timeout 60 "$B/llvm-mc" -triple i4004 --filetype=obj "$RT/$s.s" -o "$WORK/$s.o" 2>/dev/null && objs+=("$WORK/$s.o")
done
objs+=("$WORK/t.o")
timeout 180 python3 "$PACK" "${objs[@]}" -o "$WORK/img.bin" -m "$WORK/img.map" \
    --banks "$NBANKS" --common-bytes "$COMMON" --bank-bytes "$BANKW" --softcall \
    2>"$WORK/pk.log" || { cat "$WORK/pk.log" >&2; fail "banked link"; }

# Verify the reachable callees (leaf/wide) landed in banks > 15.
fhi=$(grep -E 't\.o:\.text\.(leaf|wide|addw2)\b' "$WORK/img.map" \
      | grep -oE 'bank[0-9]+' | grep -oE '[0-9]+' | sort -n | tail -1)
status "highest target-callee bank: ${fhi:-?} (need > 15 for a meaningful test)"
[[ "${fhi:-0}" -gt 15 ]] || fail "no target callee landed above bank 15 (test vacuous); raise NFILL"

got=$("$H" "$WORK/img.bin" 300000 8)
# leaf(1)=8 @char0 ; wide=0x0406 @char4 (6,0,4,0).
want="8 0 0 0 6 0 4 0"
if [[ "$got" == "$want" ]]; then
    echo "WIDE-BANK EXEC: PASS (cross-bank call to a callee in bank $fhi (>15); i16 0x0406, both via the 8-bit latch high nibble)" >&2
    exit 0
else
    echo "WIDE-BANK EXEC: got [$got] want [$want] (highest chain bank $fhi)" >&2
    exit 1
fi
