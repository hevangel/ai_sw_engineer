#!/usr/bin/env bash
# Execution-test the CROSS-BANK software-call path on `external/chips` (item 6):
# the 4-nibble return token (incl. caller bank), the per-callee __tobank set-
# latch stub (forward switch), and __soft_ret's latch restore. A bank-reloading
# chips harness watches the ROM bank latch (chip-3 output port) and swaps the
# banked ROM window on a change -- the same adapter shape that goes into
# platform/scripts/chips_trace.rs for the 42-example run.
#
# The test IR is linked with a SMALL bank window so the depth chain, the i16
# path and the recursion scatter across several banks; a correct result proves
# control + the bank latch cross every bank boundary and come back.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/build/llvm-obj/bin"; [[ -x "$B/llc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/scbx.XXXXXX")}"
mkdir -p "$WORK"
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }
[[ -x "$B/llc" ]] || fail "llc missing at $B"

COMMON=0x800
BANKW=0x50      # small banked window: ~1-2 small functions per bank, so the
               # depth-8 chain + i16 path span several banks -> real cross-bank
COMMONI=$((COMMON)); BANKI=$((BANKW))

# --- bank-reloading harness (watches the chip-3 latch) -----------------------
status "building bank-reloading chips harness"
mkdir -p "$WORK/h/src"
cat > "$WORK/h/Cargo.toml" <<EOF
[package]
name="scbx"
version="0.1.0"
edition="2021"
[[bin]]
name="scbx"
path="src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cat > "$WORK/h/src/main.rs" <<RS
// Bank-reloading chips harness. image = common(COMMON) || bank0(BANKW) ||
// bank1(BANKW) || ...  Watches the ROM bank latch (roms[3].ports) and, on a
// change, rebuilds the banked ROM chips (those whose base >= COMMON) from the
// selected bank's window, keeping common chips + RAM + CPU state intact.
use chips::mcs4::Board;
use arbitrary_int::u4;
const COMMON: usize = $COMMONI;
const BANKW: usize = $BANKI;
fn view(common: &[u8], banked: &[u8], bank: usize) -> Vec<u8> {
    let mut v = vec![0u8; 0x1000];
    let c = common.len().min(COMMON);
    v[..c].copy_from_slice(&common[..c]);
    let off = bank * BANKW;
    if off < banked.len() {
        let n = (banked.len() - off).min(BANKW);
        v[COMMON..COMMON + n].copy_from_slice(&banked[off..off + n]);
    }
    v
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let image = std::fs::read(&a[1]).unwrap();
    let cycles: usize = a[2].parse().unwrap();
    let nchars: usize = a[3].parse().unwrap();
    let common = &image[..COMMON.min(image.len())];
    let banked = if image.len() > COMMON { &image[COMMON..] } else { &[][..] };
    let first_banked = COMMON / 0x100;           // chip index where banks begin
    let mut bank = 0usize;
    let mut b = Board::new(view(common, banked, bank), 2);
    let mut prev = b.roms[3].ports.value();
    for _ in 0..cycles {
        b.run_cycle();
        let latch = b.roms[3].ports.value() as usize;
        if latch != prev as usize {
            prev = latch as u8;
            if latch != bank {
                bank = latch;
                let v = view(common, banked, bank);
                for i in first_banked..16usize {
                    if i < b.roms.len() {
                        let s = i * 0x100;
                        let chunk: [u8; 0x100] = v[s..s + 0x100].try_into().unwrap();
                        b.roms[i] = chips::mcs4::rom4001::ROM::new(chunk, u4::new(i as u8));
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for c in 0..nchars {
        out.push(b.rams[0].registers[0].characters.read_nibble(c as u8).value().to_string());
    }
    println!("{}", out.join(" "));
}
RS
( cd "$WORK/h" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/c.log" 2>&1 ) \
    || { tail -25 "$WORK/c.log" >&2; fail "harness build"; }
H="$WORK/ct/release/scbx"

# --- reset stub (soft-enter entry). ALL in the common .reset section so it is
# mapped in every bank; only the compiled chain's calls are cross-bank.
cat > "$WORK/reset.s" <<'EOF'
	.section .reset,"ax"
	.globl _start
_start:
	ldm	0
	dcl
	fim	p0, 0x0f
	src	p0
	ldm	0
	wrm			; __cur_bank = 0
	fim	p7, 0x40	; SSP base
	fim	p0, %lo8(__halt)
	jms	__push_off
	fim	p0, %page(__halt)
	jms	__push_pg
	jun	entry
	.globl __halt
__halt:	jun __halt
EOF

# --- test IR (chain depth 8 + i16 through a call) ---------------------------
cat > "$WORK/t.ll" <<'EOF'
target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"
target triple = "i4004"
define i8 @f7(i8 %x) { %r = add i8 %x, 1 ret i8 %r }
define i8 @f6(i8 %x) { %a=add i8 %x,1 %r=call i8 @f7(i8 %a) ret i8 %r }
define i8 @f5(i8 %x) { %a=add i8 %x,1 %r=call i8 @f6(i8 %a) ret i8 %r }
define i8 @f4(i8 %x) { %a=add i8 %x,1 %r=call i8 @f5(i8 %a) ret i8 %r }
define i8 @f3(i8 %x) { %a=add i8 %x,1 %r=call i8 @f4(i8 %a) ret i8 %r }
define i8 @f2(i8 %x) { %a=add i8 %x,1 %r=call i8 @f3(i8 %a) ret i8 %r }
define i8 @f1(i8 %x) { %a=add i8 %x,1 %r=call i8 @f2(i8 %a) ret i8 %r }
define i8 @chain8(i8 %x){ %a=add i8 %x,1 %r=call i8 @f1(i8 %a) ret i8 %r }
define i16 @addw2(i16 %a,i16 %b){ %r=add i16 %a,%b ret i16 %r }
define i16 @wide(i16 %a,i16 %b){ %r=call i16 @addw2(i16 %a,i16 %b) ret i16 %r }
define void @entry(){
  %c=call i8 @chain8(i8 0) store volatile i8 %c, ptr inttoptr (i16 0 to ptr)
  %w=call i16 @wide(i16 258,i16 772) store volatile i16 %w, ptr inttoptr (i16 4 to ptr)
  ret void
}
EOF

status "llc + assemble + banked --softcall link (common=$COMMON bank=$BANKW)"
timeout 120 "$B/llc" -mtriple=i4004 -function-sections -filetype=obj "$WORK/t.ll" -o "$WORK/t.o" \
    2>"$WORK/llc.log" || { cat "$WORK/llc.log" >&2; fail "llc"; }
timeout 60 "$B/llvm-mc" -triple i4004 --filetype=obj "$WORK/reset.s" -o "$WORK/reset.o" \
    2>"$WORK/mc.log" || { cat "$WORK/mc.log" >&2; fail "assemble reset"; }
objs=("$WORK/reset.o")
for s in arith arith16 bcd logic logic16 shift shift16 mem stack regops; do
    timeout 60 "$B/llvm-mc" -triple i4004 --filetype=obj "$RT/$s.s" -o "$WORK/$s.o" 2>/dev/null && objs+=("$WORK/$s.o")
done
objs+=("$WORK/t.o")
timeout 120 python3 "$PACK" "${objs[@]}" -o "$WORK/img.bin" -m "$WORK/img.map" \
    --banks 16 --common-bytes "$COMMON" --bank-bytes "$BANKW" --softcall \
    2>"$WORK/pk.log" || { cat "$WORK/pk.log" >&2; fail "banked link"; }
status "banks used: $(grep -c 'bank [0-9]*:' "$WORK/img.map"); cross-bank stubs: $(grep -c __tobank "$WORK/img.map" 2>/dev/null || echo '?')"

got=$("$H" "$WORK/img.bin" 60000 8)
# chain8(0)=8 -> 8,0 ; wide=0x0406 -> 6,0,4,0
want="8 0 0 0 6 0 4 0"
if [[ "$got" == "$want" ]]; then
    echo "SOFTCALL BANKED EXEC: PASS (chain8=8 across banks; wide=0x0406 i16 cross-bank)" >&2
    exit 0
else
    echo "SOFTCALL BANKED EXEC: got [$got] want [$want]" >&2
    sed -n '1,14p' "$WORK/img.map" >&2
    exit 1
fi
