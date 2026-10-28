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
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 015_directory_error "$D/d10_directory_error.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 016_chain_beyond_disk "$D/d11_chain_beyond_disk.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 017_chain_loop "$D/d12_chain_loop.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 018_no_clusters "$D/d13_no_clusters.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 019_one_lost_cluster "$D/d14_one_lost_cluster.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 020_chain_to_315 "$D/d15_chain_to_315.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 021_first_cluster_315 "$D/d16_first_cluster_315.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 022_end_ff8 "$D/d17_end_ff8.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 023_chain_ff7 "$D/d18_chain_ff7.img" "chkdsk b:"
"$(dirname "${BASH_SOURCE[0]}")/rtl_oracle.sh" 024_lost_last_cluster "$D/d19_lost_last_cluster.img" "chkdsk b:"
