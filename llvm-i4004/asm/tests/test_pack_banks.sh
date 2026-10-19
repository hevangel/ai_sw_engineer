#!/usr/bin/env bash
# Unit test for i4004_pack.py --banks: classification, placement, multi-bank
# emission, bank map, and cross-bank-call detection. Uses tiny hand-assembled
# objects (no firmware). Run inside the ai-sw-engineer container.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
B="/llvm-build/llvm-obj/bin"; [[ -x "$B/llvm-mc" ]] || B="$ROOT/llvm-i4004/install/bin"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
W="$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/packbank.XXXXXX")"
pass=0; fail=0
chk(){ if eval "$2"; then pass=$((pass+1)); else fail=$((fail+1)); echo "FAIL: $1" >&2; fi; }

# common: .reset + a runtime helper __rt (common by name rule).
cat > "$W/common.s" <<'EOF'
	.section .reset,"ax"
reset:	jun start
	.section .text.start,"ax"
start:	jms __rt
	bbl 0
	.section .text.__rt,"ax"
	.globl __rt
__rt:	bbl 0
EOF
# two firmware funcs (mangled -> banked). fa calls fb (tests cross-bank).
cat > "$W/fa.s" <<'EOF'
	.section .text._RNvFa,"ax"
	.globl _RNvFa
_RNvFa:	jms _RNvFb
	jms __rt
	bbl 0
EOF
cat > "$W/fb.s" <<'EOF'
	.section .text._RNvFb,"ax"
	.globl _RNvFb
_RNvFb:	bbl 0
EOF
$B/llvm-mc -triple i4004 --filetype=obj "$W/common.s" -o "$W/common.o" 2>&1 | head
$B/llvm-mc -triple i4004 --filetype=obj "$W/fa.s" -o "$W/fa.o" 2>&1 | head
$B/llvm-mc -triple i4004 --filetype=obj "$W/fb.s" -o "$W/fb.o" 2>&1 | head

# Test 1: tiny common-bytes forces runtime overflow detection.
out=$(python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" -o "$W/x.bin" --banks 2 --common-bytes 0x4 --bank-bytes 0x40 2>&1)
chk "common overflow is detected" '[[ "$out" == *"common ROM region overflow"* ]]'

# Test 2: with fa and fb forced into DIFFERENT banks (bank-bytes tiny so each
# func gets its own bank), fa->fb is a cross-bank call and must be reported.
out=$(python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" -o "$W/x.bin" --banks 2 --common-bytes 0x100 --bank-bytes 0x8 2>&1)
chk "cross-bank call detected" '[[ "$out" == *"cross-bank"* ]]'

# Test 3: fa and fb co-located in one bank (big bank) -> links, 2-bank image,
# map lists both as bank0, common holds reset/start/__rt.
python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" -o "$W/ok.bin" -m "$W/ok.map" --banks 2 --common-bytes 0x100 --bank-bytes 0x40 >/dev/null 2>"$W/e3"
chk "co-located link succeeds" '[[ -f "$W/ok.bin" ]]'
sz=$(wc -c < "$W/ok.bin")
# image = common(0x100) + 2 banks x 0x40 = 0x180 = 384
chk "banked image size = common + banks*bank" "[[ $sz -eq 384 ]]"
chk "map marks __rt common" 'grep -qE "common .*__rt" "$W/ok.map"'
chk "map marks _RNvFa bank0" 'grep -qE "bank0 .*_RNvFa" "$W/ok.map"'
chk "map marks _RNvFb bank0" 'grep -qE "bank0 .*_RNvFb" "$W/ok.map"'

# Test 4: non-banked (default) path unchanged -> flat image.
python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" -o "$W/flat.bin" --max 4096 --pad 256 >/dev/null 2>"$W/e4"
chk "flat (no --banks) still works" '[[ -f "$W/flat.bin" ]]'

# Test 5: trampoline synthesis. Force fa and fb into different banks (tiny bank),
# emit trampolines, assemble, and rewrite-link. fa->fb is cross-bank so a
# __bankcall_<fb> trampoline must be generated and the link must succeed.
TB="--banks 2 --common-bytes 0x100 --bank-bytes 0x8"
python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" $TB --emit-trampolines "$W/tr.s" >/dev/null 2>"$W/e5a"
chk "emit-trampolines writes a file" '[[ -s "$W/tr.s" ]]'
chk "trampoline generated for the cross-bank callee" 'grep -qE "__bankcall_[0-9a-f]+" "$W/tr.s"'
$B/llvm-mc -triple i4004 --filetype=obj "$W/tr.s" -o "$W/tr.o" 2>"$W/e5b"
chk "trampoline source assembles" '[[ -f "$W/tr.o" ]]'
python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" "$W/tr.o" $TB --rewrite-cross-bank -o "$W/tr.bin" >/dev/null 2>"$W/e5c"
chk "rewrite-cross-bank link succeeds" '[[ -f "$W/tr.bin" ]]'
chk "without trampolines, cross-bank still errors" '! python3 "$PACK" "$W/common.o" "$W/fa.o" "$W/fb.o" $TB -o "$W/no.bin" >/dev/null 2>&1'

# Test 6: page-align. A banked function whose relaxed conditional branch
# (JCN !cc,.L ; JUN far) is positioned so the near JCN straddles a 256-byte page
# -- the packer must NOP-pad it off the page end and link cleanly. We force this
# with a filler to push the branch near a page boundary, using the real
# relaxation the backend emits. (A pure-asm JCN-at-0xFE: PAGE8 to a same-section
# label one byte into the next page.)
cat > "$W/pg.s" <<'EOF'
	.section .text._RNvPg,"ax"
	.globl _RNvPg
_RNvPg:
	.space 0xfd
	jcn	0x4, .Lt   ; opcode at 0xfd, operand 0xfe, target .Lt one byte into next page
	jun	_RNvPg
.Lt:
	bbl 0
EOF
$B/llvm-mc -triple i4004 --filetype=obj "$W/pg.s" -o "$W/pg.o" 2>"$W/e6"
chk "page-straddle test obj assembles" '[[ -f "$W/pg.o" ]]'
# bank it (1 KB bank so it is in the banked region, > 256 B so it spans pages);
# the page-align pass must pad the JCN off 0xfd and the link must succeed.
python3 "$PACK" "$W/common.o" "$W/pg.o" --banks 1 --common-bytes 0x100 --bank-bytes 0x400 -o "$W/pg.bin" >/dev/null 2>"$W/e6b"
chk "page-align lets a page-straddling JCN link" '[[ -f "$W/pg.bin" ]]'

echo "PACK-BANK UNIT: $pass passed, $fail failed" >&2
[[ "$fail" -eq 0 ]]
