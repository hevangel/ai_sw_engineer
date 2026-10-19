#!/usr/bin/env bash
# Exec-test CM-RAM bank switching on the chips model: write 4 distinct values to
# the SAME SRC address (0x00) under DCL banks 0,1,2,4 (the single-CM-RAM-line
# codes: CM-RAM0=0, CM-RAM1=1, CM-RAM2=2, CM-RAM3=4 per the 4004 DCL table),
# then read each bank back. Proves the 16-chip RAM layout (4 banks x 4 chips,
# page_mask = i%4) gives independent per-bank storage with the UNMODIFIED chips
# model (external/ untouched; we build b.rams ourselves via the public field).
set -uo pipefail
ROOT=/workspace
CH=$ROOT/external/chips
B="/build/llvm-obj/bin"; [[ -x "$B/llvm-mc" ]] || B="$ROOT/llvm-i4004/install/bin"
W=$(mktemp -d ${KIROCREW_SCRATCH:-/tmp}/dclt.XXXXXX); mkdir -p $W/h/src
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }

# Program: for each bank b in {0,1,2,4}: LDM b; DCL; FIM p0,0; SRC p0; LDM v; WRM.
# Values: bank0->3, bank1->5, bank2->9, bank4->0xC. Then read back bank0..:
#   read phase writes results to bank 0 reg1 chars 0..3 (bank0 so the harness
#   can read them without DCL games): for each b: LDM b;DCL; FIM p0,0;SRC;RDM;
#   -> ACC ; LDM0;DCL; FIM p0,0x10+i;SRC;LD acc? can't LD acc -> stash via xch.
# Simpler: after writing all 4 banks, read each into a register then store to
# bank0 reg1. Use R2..R5 to hold the 4 read values.
cat > $W/t.s <<'EOF'
	.section .reset,"ax"
	.globl _start
_start:
	; write distinct values to SRC 0x00 in banks 0,1,2,4
	ldm 0
	dcl
	fim p0, 0x00
	src p0
	ldm 3
	wrm
	ldm 1
	dcl
	fim p0, 0x00
	src p0
	ldm 5
	wrm
	ldm 2
	dcl
	fim p0, 0x00
	src p0
	ldm 9
	wrm
	ldm 4
	dcl
	fim p0, 0x00
	src p0
	ldm 12
	wrm
	; read each bank's SRC 0x00 into R2,R3,R4,R5
	ldm 0
	dcl
	fim p0, 0x00
	src p0
	rdm
	xch r2
	ldm 1
	dcl
	fim p0, 0x00
	src p0
	rdm
	xch r3
	ldm 2
	dcl
	fim p0, 0x00
	src p0
	rdm
	xch r4
	ldm 4
	dcl
	fim p0, 0x00
	src p0
	rdm
	xch r5
	; store R2..R5 to bank 0, SRC 0x10..0x13 (reg1 chars 0..3)
	ldm 0
	dcl
	fim p0, 0x10
	src p0
	ld r2
	wrm
	fim p0, 0x11
	src p0
	ld r3
	wrm
	fim p0, 0x12
	src p0
	ld r4
	wrm
	fim p0, 0x13
	src p0
	ld r5
	wrm
.Lhalt:	jun .Lhalt
EOF
$B/llvm-mc -triple i4004 --filetype=obj $W/t.s -o $W/t.o 2>$W/mc.log || { cat $W/mc.log; fail "assemble"; }
# link flat (tiny): just emit the raw bytes. Use the packer with no banks.
python3 $ROOT/llvm-i4004/asm/i4004_pack.py $W/t.o -o $W/img.bin --max 4096 2>$W/pk.log || { cat $W/pk.log; fail "link"; }

cat > $W/h/Cargo.toml <<EOF
[package]
name="dclt"
version="0.1.0"
edition="2021"
[[bin]]
name="dclt"
path="src/main.rs"
[dependencies]
chips = { path = "$CH" }
arbitrary-int = "=1.3.0"
EOF
cat > $W/h/src/main.rs <<'RS'
use chips::mcs4::Board; use arbitrary_int::u2;
fn main(){
  let mut img=std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
  while img.len() % 0x100 != 0 { img.push(0); }
  if img.is_empty() { img.resize(0x100, 0); }
  let mut b=Board::new(img, 0);
  // 16 RAM chips = 4 CM-RAM banks x 4 chips; page_mask = chip-within-bank = i%4.
  b.rams.clear();
  for i in 0..16u8 { b.rams.push(chips::mcs4::ram4002::RAM::new(u2::new(i%4))); }
  for _ in 0..4000 { b.run_cycle(); }
  // results stored in bank 0 (chips 0-3), chip0 reg1 chars 0..3 (SRC 0x10..0x13)
  let r=&b.rams[0].registers[1].characters;
  let v:Vec<u8>=(0..4).map(|c| r.read_nibble(c as u8).value()).collect();
  println!("{} {} {} {}", v[0],v[1],v[2],v[3]);
}
RS
( cd $W/h && CARGO_TARGET_DIR=$W/ct cargo build --release >$W/c.log 2>&1 ) || { tail -20 $W/c.log; fail "harness"; }
got=$($W/ct/release/dclt $W/img.bin)
want="3 5 9 12"
if [[ "$got" == "$want" ]]; then
  echo "RAM DCL EXEC: PASS (banks 0,1,2,4 independent: [$got])" >&2
  exit 0
else
  echo "RAM DCL EXEC: got [$got] want [$want]" >&2
  exit 1
fi
