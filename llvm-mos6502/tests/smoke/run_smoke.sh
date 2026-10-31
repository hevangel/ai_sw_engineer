#!/usr/bin/env bash
# Smoke test for llvm-mos6502 Rust toolchain.
# Compiles a minimal no_std binary and executes it on the mos-sim simulator.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
WORK="$(mktemp -d "/tmp/mos_smoke.XXXXXX")"

trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/src" "$WORK/.cargo"

cat << 'EOF' > "$WORK/.cargo/config.toml"
[build]
target = "mos-sim-none"

[unstable]
build-std = ["core"]
EOF

cat << 'EOF' > "$WORK/Cargo.toml"
[package]
name = "mos_smoke"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mos_smoke"
path = "src/main.rs"

[profile.release]
panic = "abort"
lto = true
EOF

cat << 'EOF' > "$WORK/src/main.rs"
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn main() -> u8 {
    let a: u8 = 20;
    let b: u8 = 22;
    a.wrapping_add(b)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
EOF

cd "$WORK"
cargo +mos build --release

BIN=$(find "${CARGO_TARGET_DIR:-/workspace/target}"/mos-sim-none/release/mos_smoke "$WORK"/target/mos-sim-none/release/mos_smoke 2>/dev/null | head -n1)
[[ -n "$BIN" && -f "$BIN" ]] || { echo "ERROR: mos_smoke binary not produced" >&2; exit 1; }

echo "==> mos_smoke binary successfully built: $(ls -lh "$BIN")"
echo "==> executing on mos-sim:"

set +e
mos-sim "$BIN"
RET=$?
set -e

if [[ $RET -eq 42 ]]; then
    echo "==> mos-sim returned 42 as expected! Test PASSED."
    exit 0
else
    echo "ERROR: unexpected mos-sim exit code $RET (expected 42)" >&2
    exit 1
fi
