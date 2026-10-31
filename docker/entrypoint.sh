#!/usr/bin/env bash
# Entry point of the full toolchain image (docker-compose.yml).
#
# The backend scripts look for their tools in <repo>/llvm-<chip>/install/bin. The
# image carries prebuilt installs under $AI_SW_TOOLCHAINS; when the bind-mounted
# repository has no install of its own, link the image's copy there (the link is
# gitignored). A locally built install/ always wins. AI_SW_LINK_TOOLCHAINS=0 disables it.
set -euo pipefail
workspace="${AI_SW_WORKSPACE:-/workspace}"
toolchains="${AI_SW_TOOLCHAINS:-/opt/ai-sw}"
if [[ ${AI_SW_LINK_TOOLCHAINS:-1} == 1 && -d $workspace ]]; then
    for chip in i4004 i8008 i8086; do
        src="$toolchains/llvm-$chip/install"
        dst="$workspace/llvm-$chip/install"
        [[ -d $src && -d $workspace/llvm-$chip ]] || continue
        if [[ -L $dst && $(readlink "$dst") == "$src" ]]; then
            continue
        elif [[ ! -e $dst && ! -L $dst ]]; then
            ln -s "$src" "$dst" 2>/dev/null \
                && echo "ai-sw: llvm-$chip/install -> $src (image toolchain)" >&2 \
                || echo "ai-sw: cannot link llvm-$chip/install (read-only mount?); use $src/bin" >&2
        fi
    done
fi
exec "$@"
