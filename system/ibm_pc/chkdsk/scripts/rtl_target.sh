#!/usr/bin/env bash
# Run the Rust CHKDSK.COM (bin/chkdsk.com) under PC DOS 1.00 on the RTL for the cases of
# oracle/transcripts and record oracle/rust_on_rtl/<case>.txt, to be compared with the original's.
#
# Usage: scripts/rtl_target.sh [case...]    (default: a set covering every kind of repair)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
HW="$(cd "$AI_HW_ENGINEER" && pwd)"
PCDISK="$PLATFORM_DIR/tools/pcdisk/target/release/pcdisk"
[[ -x $PCDISK ]] || (cd "$PLATFORM_DIR/tools/pcdisk" && cargo build --quiet --release)
mkdir -p "$BUILD_DIR/disks" "$BUILD_DIR/rtl_target" "$TITLE_DIR/oracle/rust_on_rtl"
DOS="$BUILD_DIR/rtl_target/dos_rchkdsk.img"
cp "$HW/system/ibm_pc/work/reference/pcdos100.img" "$DOS"
"$PCDISK" add "$DOS" "$TITLE_DIR/bin/chkdsk.com" RCHKDSK.COM
disk_for() { # transcript name -> disk image
    case $1 in
        001_workdisk) echo "$HW/system/ibm_pc/work/reference/workdisk.img" ;;
        003_dosdisk) echo "$HW/system/ibm_pc/work/reference/pcdos100.img" ;;
        002_blank) echo "$BUILD_DIR/blank.img" ;;
        004_sizes|012_bad_drive|013_bad_parameter|014_bad_parameter_name) echo "$BUILD_DIR/disks/d3_sizes.img" ;;
        005_size_error) echo "$BUILD_DIR/disks/d4_size_error.img" ;;
        006_allocation_error) echo "$BUILD_DIR/disks/d5_allocation_error.img" ;;
        007_cross_linked) echo "$BUILD_DIR/disks/d6_cross_linked.img" ;;
        008_lost_chain) echo "$BUILD_DIR/disks/d7_lost_chain.img" ;;
        015_directory_error) echo "$BUILD_DIR/disks/d10_directory_error.img" ;;
        022_end_ff8) echo "$BUILD_DIR/disks/d17_end_ff8.img" ;;
        024_lost_last_cluster) echo "$BUILD_DIR/disks/d19_lost_last_cluster.img" ;;
        017_chain_loop) echo "$BUILD_DIR/disks/d12_chain_loop.img" ;;
        *) fail "no disk for $1" ;;
    esac
}
cases=("$@")
[[ ${#cases[@]} -gt 0 ]] || cases=(003_dosdisk 005_size_error 006_allocation_error 007_cross_linked 008_lost_chain 015_directory_error 017_chain_loop 012_bad_drive)
for c in "${cases[@]}"; do
    cmd=$(sed -n 's/^> //p' "$TITLE_DIR/oracle/transcripts/$c.txt" | head -n1)
    DOS_IMG="$DOS" OUT_DIR="$TITLE_DIR/oracle/rust_on_rtl" "$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" "$c" "$(disk_for "$c")" "r$cmd"
done
