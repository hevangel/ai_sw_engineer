#!/usr/bin/env bash
# busicom_141pf HAL glue execution tests on the bare `chips` MCS-4 model.
# Runs inside the ai-sw-engineer container.
#
#   docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
#     ai-sw-engineer:latest bash system/busicom_141pf/platform/runtime/tests/run_hal_tests.sh
#
# Tests each of the four HAL glue functions with a hand-written stub, driving
# the TEST pin where relevant, and checks an observable (RAM/ROM port or a
# stored return nibble) against a Rust-computed oracle.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
RTDIR="$ROOT/system/busicom_141pf/platform/runtime"
LLVMRT="$ROOT/llvm-i4004/rt"
B="/llvm-build/llvm-obj/bin"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
W="$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/hal_test.XXXXXX")"
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }

[[ -x "$B/llvm-mc" ]] || fail "llvm-mc missing at $B"

# build hal_test harness
status "building hal_test harness"
mkdir -p "$W/h/src"
cat > "$W/h/Cargo.toml" <<EOF
[package]
name = "hal-test"
version = "0.1.0"
edition = "2021"
[[bin]]
name = "hal_test"
path = "src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cp "$RTDIR/tests/hal_test.rs" "$W/h/src/main.rs"
( cd "$W/h" && CARGO_TARGET_DIR="$W/ct" cargo build --release >"$W/cargo.log" 2>&1 ) \
    || { tail -30 "$W/cargo.log" >&2; fail "hal_test build failed"; }
HARNESS="$W/ct/release/hal_test"

$B/llvm-mc -triple i4004 --filetype=obj "$RTDIR/hal.s" -o "$W/hal.o"

pass=0; failc=0
chk(){ if [ "$1" = "$2" ]; then pass=$((pass+1)); else failc=$((failc+1)); echo "MISMATCH $3: got $1 want $2" >&2; fi; }

link(){ # stub.s -> image.bin
  $B/llvm-mc -triple i4004 --filetype=obj "$1" -o "$W/stub.o"
  python3 "$PACK" "$W/stub.o" "$W/hal.o" -o "$W/img.bin" --pad-pages --pad 1280 >/dev/null 2>&1
}

status "=== __bus_test_pin: both pin levels ==="
# stub: call __bus_test_pin, store R3 (result) to RAM0 reg0 char0
cat > "$W/tp.s" <<EOF
	.section .reset,"ax"
	jun S
	.text
S:	jms __bus_test_pin
	fim p0, 0x00
	src p0
	ld r3
	wrm
H:	jun H
EOF
link "$W/tp.s"
# chips set_test_flag(true) == pin LOW (manual/RTL) -> __bus_test_pin returns 0
g=$("$HARNESS" "$W/img.bin" 2000 1 ram_char 0 0); chk "$g" 0 "__bus_test_pin (chips flag=1 => pin LOW => 0)"
g=$("$HARNESS" "$W/img.bin" 2000 0 ram_char 0 0); chk "$g" 1 "__bus_test_pin (chips flag=0 => pin HIGH => 1)"

status "=== __bus_ram_port_write: RAM0 and RAM1 ports ==="
for cw in "0 7" "1 5" "0 10" "1 15"; do
  set -- $cw; chip=$1; val=$2
  cat > "$W/rw.s" <<EOF
	.section .reset,"ax"
	jun S
	.text
S:	fim p1, 0x0$(printf %x $chip)
	fim p2, 0x$(printf %02x $val)
	jms __bus_ram_port_write
H:	jun H
EOF
  link "$W/rw.s"
  g=$("$HARNESS" "$W/img.bin" 2000 0 ram_port $chip); chk "$g" "$val" "__bus_ram_port_write chip=$chip val=$val"
done

status "=== __bus_rom_port_write: ROM port 0/1/2 ==="
for cw in "0 3" "1 9" "2 6"; do
  set -- $cw; chip=$1; val=$2
  cat > "$W/rpw.s" <<EOF
	.section .reset,"ax"
	jun S
	.text
S:	fim p1, 0x0$(printf %x $chip)
	fim p2, 0x$(printf %02x $val)
	jms __bus_rom_port_write
H:	jun H
EOF
  link "$W/rpw.s"
  g=$("$HARNESS" "$W/img.bin" 2000 0 rom_port $chip); chk "$g" "$val" "__bus_rom_port_write chip=$chip val=$val"
done

status "=== __bus_rom_port_read: write via WRR then read back ==="
# stub: write 0xA to ROM port 2 via glue, then read it back via glue, store to RAM0 char0
cat > "$W/rpr.s" <<EOF
	.section .reset,"ax"
	jun S
	.text
S:	fim p1, 0x02
	fim p2, 0x0A
	jms __bus_rom_port_write
	fim p1, 0x02
	jms __bus_rom_port_read
	fim p0, 0x00
	src p0
	ld r3
	wrm
H:	jun H
EOF
link "$W/rpr.s"
g=$("$HARNESS" "$W/img.bin" 2000 0 ram_char 0 0); chk "$g" 10 "__bus_rom_port_read (wrote 0xA to ROM2, read back)"

echo "" >&2
status "HAL RESULT: $pass passed, $failc failed"
[[ "$failc" == 0 ]] || exit 1
