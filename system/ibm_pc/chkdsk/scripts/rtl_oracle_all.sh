#!/usr/bin/env bash
# Build the damaged test disks and record the original CHKDSK on every case (about 3 minutes each).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
D="$BUILD_DIR/disks"
mkdir -p "$D"
MSYS_NO_PATHCONV=1 docker run --rm -v "$TITLE_DIR:/title" "${DOCKER_IMAGE:-ai-hw-engineer:latest}" \
    python3 /title/scripts/make_disks.py /title/build/disks
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 004_sizes "$D/d3_sizes.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 005_size_error "$D/d4_size_error.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 006_allocation_error "$D/d5_allocation_error.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 007_cross_linked "$D/d6_cross_linked.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 008_lost_chain "$D/d7_lost_chain.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 009_full "$D/d8_full.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 010_media_byte "$D/d9_media_byte.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 011_no_args "$D/d3_sizes.img" "chkdsk"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 012_bad_drive "$D/d3_sizes.img" "chkdsk c:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 013_bad_parameter "$D/d3_sizes.img" "chkdsk b: x"
