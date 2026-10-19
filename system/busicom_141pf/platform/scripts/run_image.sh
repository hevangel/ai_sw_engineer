#!/usr/bin/env bash
# busicom_141pf platform image runner.
#
# Runs ANY 4004 ROM image (1280..4096 bytes) on a whole-machine simulator and
# emits the port-level tape in EXACTLY oracle/rom_port_trace.txt format for all
# 42 manual examples. Two simulators, fed byte-identical stimulus:
#
#   primary    -> ai_hw_engineer Busicom RTL board, Verilated (the authoritative
#                 hardware model). Needs Verilator (the ai-hw-engineer:latest
#                 image has it; the ai-sw-engineer image does not).
#   secondary  -> external/chips V. Ilmer MCS-4 board model (independent oracle).
#
# Both decode the printed tape with the SAME drum tables and the SAME drum/TEST/
# index pacing, so their output is directly comparable; any disagreement is a
# FINDING (AGENTS.md), not noise.
#
# Usage:
#   run_image.sh primary   <image.bin> <out_trace.txt> [schedule_or_json] [--board stock|xl]
#   run_image.sh secondary <image.bin> <out_trace.txt> [schedule_or_json] [--board stock|xl]
#
# The third argument is the key/switch stimulus. Default: the committed
# oracle/manual_examples.json (all 42 examples). A .json is converted to a
# schedule by build_schedule.py; a non-.json file is used as a schedule verbatim.
#
# --board selects the hardware variant (default: stock):
#   stock  5 x 4001 ROM (1280 B) + 2 x 4002 RAM (bank 0), the original board.
#   xl     16 x 4001 ROM (4096 B) + 16 x 4002 RAM (4 banks), the extended board
#          (platform/rtl/busicom_141pf_xl.sv, link_xl.ld, memory_map.md "xl").
#          Busicom I/O is identical; only memory is added. The original ROM
#          reproduces the oracle byte-for-byte on either board; xl exists for
#          the compiled Rust firmware, which does not fit the stock board.
#
# Env:
#   AI_HW_ENGINEER   path to the ai_hw_engineer repo (default: repo's ../ai_hw_engineer)
#   CHIPS_DIR        path to the pinned external/chips checkout (default:
#                    $ROOT/external/chips; falls back to $AI_HW_ENGINEER/.. layout)
#   BUSICOM_SPIN     RTL machine cycles per drum half-spin (default 1481, the
#                    reference value the oracle was captured at; smaller is faster
#                    and prints identical rows down to ~740, below which the
#                    firmware's per-sector print burst no longer completes)
#   WORK             scratch dir (default: a mktemp under ${KIROCREW_SCRATCH:-/tmp})
set -euo pipefail

sim="${1:?primary|secondary}"
image="${2:?image.bin}"
out="${3:?out_trace.txt}"
shift 3 || true

stim=""
board="stock"
while (( $# )); do
    case "$1" in
    --board) board="${2:?--board needs stock|xl}"; shift 2 ;;
    --board=*) board="${1#--board=}"; shift ;;
    *) if [[ -z $stim ]]; then stim="$1"; shift; else echo "unexpected arg: $1" >&2; exit 2; fi ;;
    esac
done
case "$board" in stock|xl) ;; *) echo "ERROR: unknown --board '$board' (stock|xl)" >&2; exit 2 ;; esac

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
platform_dir="$(cd "$here/.." && pwd)"
system_dir="$(cd "$platform_dir/.." && pwd)"
root="$(cd "$system_dir/../.." && pwd)"
title_dir="$system_dir/firmware"

AI_HW_ENGINEER="${AI_HW_ENGINEER:-$root/../ai_hw_engineer}"
CHIPS_DIR="${CHIPS_DIR:-$root/external/chips}"
BUSICOM_SPIN="${BUSICOM_SPIN:-1481}"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/busicom_run.XXXXXX")}"

status() { printf '==> %s\n' "$*" >&2; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

[[ -f $image ]] || fail "image not found: $image"
sz=$(wc -c < "$image")
# A flat image is 1280..4096 B. A BANKED image (i4004_pack --banks, common +
# N*bank) is larger and only runs on the secondary (chips) sim, whose
# chips_trace adapter reloads banked ROM on the chip-3 latch; the primary (RTL)
# sim splits into <=16 physical 4001s so it is capped at 4096 B.
if (( sz > 4096 )); then
    [[ $sim == secondary ]] || fail "banked image ($sz B > 4096) runs on the secondary sim only; the RTL board has <=16 4001s (4096 B)"
    (( sz <= 0x20000 )) || fail "image size $sz too large (banked max 128 KiB)"
else
    (( sz >= 1280 && sz <= 4096 )) || fail "image size $sz out of range (1280..4096)"
fi

mkdir -p "$WORK"

# ---- build the schedule -----------------------------------------------------
schedule="$WORK/schedule.txt"
if [[ -z $stim ]]; then
    stim="$title_dir/oracle/manual_examples.json"
fi
if [[ $stim == *.json ]]; then
    python3 "$here/build_schedule.py" "$stim" -o "$schedule"
else
    cp "$stim" "$schedule"
fi

case "$sim" in
primary)
    command -v verilator >/dev/null || fail "verilator not on PATH (run in the ai-hw-engineer image)"
    [[ -d $AI_HW_ENGINEER/design/intel_4004 ]] || fail "ai_hw_engineer RTL not found at $AI_HW_ENGINEER"
    des="$AI_HW_ENGINEER/design"
    sysrtl="$AI_HW_ENGINEER/system/busicom_141pf"
    # split image into the 256-byte 4001 masks the board reads from ./src/rom
    mkdir -p "$WORK/src/rom"
    if [[ $board == xl ]]; then
        python3 "$here/split_rom.py" "$image" "$WORK/src/rom" --chips 16
    else
        python3 "$here/split_rom.py" "$image" "$WORK/src/rom"
    fi
    # select the board top. For xl, generate a board-select header the harness
    # includes (avoids -D string-literal quote mangling through make).
    if [[ $board == xl ]]; then
        top="busicom_141pf_xl"
        top_sv="$here/../rtl/busicom_141pf_xl.sv"
        cat > "$WORK/board_sel.h" <<EOF
#define BOARD_TOP busicom_141pf_xl
#define BOARD_HDR "Vbusicom_141pf_xl.h"
#define BOARD_ROOT_HDR "Vbusicom_141pf_xl___024root.h"
#define LAMP_MEMBER busicom_141pf_xl__DOT__ram1_port4
EOF
        cflags="-DBOARD_SEL_HEADER -I$WORK"
    else
        top="busicom_141pf"
        top_sv="$sysrtl/src/busicom_141pf.sv"
        cflags=""
    fi
    status "verilating board '$board' (top=$top, Verilator)"
    ( cd "$WORK" && verilator --cc --exe --build -O3 -Wno-fatal --public-flat-rw \
        --top-module "$top" --Mdir "$WORK/obj_dir" -j 4 \
        ${cflags:+-CFLAGS "$cflags"} \
        "$des/intel_4004/src/intel_4004.sv" \
        "$des/intel_4001/src/intel_4001.sv" \
        "$des/intel_4002/src/intel_4002.sv" \
        "$des/intel_4003/src/intel_4003.sv" \
        "$top_sv" \
        "$here/tb_trace_main.cpp" >"$WORK/verilate.log" 2>&1 ) \
        || { tail -30 "$WORK/verilate.log" >&2; fail "verilation failed"; }
    status "running RTL (spin=$BUSICOM_SPIN; this is slow, minutes)"
    ( cd "$WORK" && BUSICOM_SPIN="$BUSICOM_SPIN" "./obj_dir/V$top" "$schedule" ) > "$out"
    ;;
secondary)
    command -v cargo >/dev/null || fail "cargo not on PATH"
    [[ -f $CHIPS_DIR/src/mcs4/cpu4004.rs ]] || fail "pinned external/chips not found at $CHIPS_DIR (git submodule update --init external/chips)"
    mkdir -p "$WORK/chips_trace/src"
    cat > "$WORK/chips_trace/Cargo.toml" <<EOF
[package]
name = "chips-trace"
version = "0.1.0"
edition = "2021"
[[bin]]
name = "chips_trace"
path = "src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
    cp "$here/chips_trace.rs" "$WORK/chips_trace/src/main.rs"
    status "building chips adapter (cargo)"
    ( cd "$WORK/chips_trace" && CARGO_TARGET_DIR="$WORK/ct_target" cargo build --release >"$WORK/cargo.log" 2>&1 ) \
        || { tail -20 "$WORK/cargo.log" >&2; fail "chips adapter build failed"; }
    # RAM 4002 count. stock = 2 (bank 0 chips 0,1).
    # xl: the DCL-banked firmware places per-function frames in CM-RAM banks
    # 1-3, so the secondary (chips) model must instantiate all 16 4002s (4 banks
    # x 4 chips). Board::new cannot build > 4 (it sets page_mask = u2::new(raw
    # index), which panics for index >= 4), so chips_trace.rs HAND-BUILDS the
    # chips for ram_count > 4 with page_mask = chip-within-bank = i % 4 (the same
    # workaround tests/exec/run_ramdcl_exec and run_frame_exec use). external/
    # chips is never edited. rams[0]/[1] keep the printer/lamp port roles, so the
    # decoded tape is unchanged.
    if [[ $board == xl ]]; then ram_count=16; else ram_count=2; fi
    status "running chips (board=$board, ram_count=$ram_count)"
    "$WORK/ct_target/release/chips_trace" "$image" "$schedule" "$ram_count" > "$out"
    ;;
*)
    fail "unknown sim '$sim' (expected primary|secondary)"
    ;;
esac

status "wrote $out ($(grep -c '^#EXAMPLE' "$out") examples)"
