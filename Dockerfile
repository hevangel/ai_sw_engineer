# AI Software Engineer toolchain image.
#
# Contents
#   * llvm-mos + llvm-mos-sdk + rust-mos (6502 family) from the pinned upstream
#     image mrkits/rust-mos (built from external/rust-mos/docker/).
#   * Host Rust (pinned stable) for host builds/tests, plus a pinned nightly with
#     rust-src for -Zbuild-std against this repo's target JSONs.
#   * Everything needed to build this repo's own LLVM backends (llvm-<chip>/) and
#     a rustc linked against them: clang, lld, cmake, ninja, python3 + lit.
#
# This repo's backends are NOT baked in: they are built inside the container by
# llvm-<chip>/scripts/build_llvm.sh into llvm-<chip>/install/ on the bind mount,
# so a rebuild survives container restarts and does not require a new image.
#
#   docker build -t ai-sw-engineer .
#   docker run --rm -it -v "$PWD:/workspace" -v "$PWD/../ai_hw_engineer:/ai_hw_engineer" ai-sw-engineer
#
# The later stages (target `full`, built by docker-compose.yml) additionally bake
# this repo's own backends and the i8086 rustc into one image; see the end of
# this file and docs/docker/README.md. The default target stays this base image.

ARG RUST_MOS_IMAGE=mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8
FROM ${RUST_MOS_IMAGE} AS base

ARG RUST_MOS_IMAGE
ARG RUST_STABLE=1.99.1
ARG RUST_NIGHTLY=nightly-2026-10-15

USER root
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl git git-lfs \
      clang lld cmake ninja-build ccache \
      binutils-m68k-linux-gnu \
      python3 python3-pip python3-venv \
      xxd file less zstd \
 && rm -rf /var/lib/apt/lists/*

# lit + FileCheck driver for backend tests (FileCheck itself comes from LLVM builds).
RUN python3 -m pip install --no-cache-dir lit==18.1.8 capstone==5.0.7

USER mos
ENV PATH=/home/mos/.cargo/bin:/usr/local/bin:${PATH} \
    RUST_TARGET_PATH=/usr/local/rust-mos/targets \
    AI_HW_ENGINEER=/ai_hw_engineer \
    CARGO_TARGET_DIR=/workspace/target

# The base image links the rust-mos toolchain as `mos`. Add host + nightly toolchains.
RUN rustup toolchain install ${RUST_STABLE} --profile minimal -c rustfmt,clippy \
 && rustup toolchain install ${RUST_NIGHTLY} --profile minimal -c rust-src,rustfmt,llvm-tools \
 && rustup default ${RUST_STABLE} \
 && rustup toolchain list

# Record versions for reports.
RUN { echo "rust-mos image: ${RUST_MOS_IMAGE}"; rustc --version; rustc +${RUST_NIGHTLY} --version; \
      rustc +mos --version; clang --version | head -n1; cmake --version | head -n1; } \
    | tee /home/mos/TOOLCHAIN_VERSIONS.txt

WORKDIR /workspace
CMD ["bash"]

# ---------------------------------------------------------------------------
# Full toolchain image: `docker compose build` (target `full`).
#
# Builds every toolchain this repository owns into one image, in parallel
# BuildKit stages, with the backends' own build scripts:
#   llvm-i4004, llvm-i8008, llvm-i8086  llc/llvm-mc/... with each experimental target
#   rust-i8086                          LLVM libraries (X86 + I8086) and a stage-1 rustc
#                                       with the i8086-unknown-none target
#   llvm-mos                            llvm-mos (clang, lld, compiler-rt) and llvm-mos-sdk
#                                       at the external/llvm-mos{,-sdk} submodule pins,
#                                       installed in /opt/ai-sw/llvm-mos
# The base stage keeps the upstream rust-mos image's own llvm-mos on PATH, because
# `rustc +mos` (and its LTO bitcode) is built against that LLVM; the pinned build
# is the repository's llvm-mos, used through $LLVM_MOS_HOME. The M68k route
# (pinned nightly + binutils-m68k) comes with the base stage. llvm-i286,
# llvm-m6800 and llvm-m6809 are audit-only and have nothing to build yet.
#
# LLVM_PROJECT_COMMIT, LLVM_MOS_COMMIT and LLVM_MOS_SDK_COMMIT must equal the
# submodule pins; scripts/compose_build.sh reads them from git and passes them in.

FROM base AS llvm-source
ARG LLVM_PROJECT_COMMIT=0d261d1ca552c95a8f007e061c787ac7132fbcbc
USER root
# Fetch exactly the pinned commit (the submodule is not needed in the build context).
RUN mkdir -p /src/external/llvm-project /build \
 && cd /src/external/llvm-project \
 && git init -q . \
 && git remote add origin https://github.com/llvm/llvm-project.git \
 && git fetch -q --depth 1 origin "${LLVM_PROJECT_COMMIT}" \
 && git checkout -q FETCH_HEAD \
 && test "$(git rev-parse HEAD)" = "${LLVM_PROJECT_COMMIT}" \
 && rm -rf .git
# The backend scripts take their repository root and scratch volume from these.
ENV WORKSPACE=/src BUILD_ROOT=/build CCACHE_DIR=/ccache

FROM llvm-source AS llvm-i4004
ARG JOBS=8
COPY llvm-i4004 /src/llvm-i4004
RUN --mount=type=cache,id=ai-sw-ccache-i4004,target=/ccache \
    JOBS="${JOBS}" bash /src/llvm-i4004/scripts/build_llvm.sh

FROM llvm-source AS llvm-i8008
ARG JOBS=8
COPY llvm-i8008 /src/llvm-i8008
RUN --mount=type=cache,id=ai-sw-ccache-i8008,target=/ccache \
    JOBS="${JOBS}" bash /src/llvm-i8008/scripts/build_llvm.sh

FROM llvm-source AS llvm-i8086
ARG JOBS=8
COPY llvm-i8086 /src/llvm-i8086
RUN --mount=type=cache,id=ai-sw-ccache-i8086,target=/ccache \
    JOBS="${JOBS}" bash /src/llvm-i8086/scripts/build_llvm.sh

# llvm-mos and its SDK, built as their READMEs describe (clang/cmake/caches/MOS.cmake,
# then the SDK against that install), with the host gcc: the base image's bare
# `clang` is the llvm-mos cross compiler and cannot build host binaries.
FROM base AS llvm-mos
ARG LLVM_MOS_COMMIT=a5c31b24a97cc16c582d124167e29e3da8946581
ARG LLVM_MOS_SDK_COMMIT=555253f65a0fa3b4bbe8a203fe21f160da27f931
ARG JOBS=8
USER root
ENV CCACHE_DIR=/ccache CC=gcc CXX=g++
RUN for repo in llvm-mos:${LLVM_MOS_COMMIT} llvm-mos-sdk:${LLVM_MOS_SDK_COMMIT}; do \
      name="${repo%%:*}"; commit="${repo#*:}"; \
      mkdir -p "/src/$name" && cd "/src/$name" && git init -q . \
      && git remote add origin "https://github.com/llvm-mos/$name.git" \
      && git fetch -q --depth 1 origin "$commit" && git checkout -q FETCH_HEAD \
      && test "$(git rev-parse HEAD)" = "$commit" && rm -rf .git || exit 1; \
    done
RUN --mount=type=cache,id=ai-sw-ccache-llvm-mos,target=/ccache \
    cmake -C /src/llvm-mos/clang/cmake/caches/MOS.cmake -G Ninja \
      -S /src/llvm-mos/llvm -B /build/llvm-mos-obj \
      -DCMAKE_INSTALL_PREFIX=/opt/ai-sw/llvm-mos \
      -DCMAKE_C_COMPILER=gcc -DCMAKE_CXX_COMPILER=g++ \
      -DLLVM_CCACHE_BUILD=ON -DLLVM_PARALLEL_LINK_JOBS=2 \
 && cmake --build /build/llvm-mos-obj -j "${JOBS}" --target install-distribution \
 && cmake -G Ninja -S /src/llvm-mos-sdk -B /build/llvm-mos-sdk-obj \
      -DCMAKE_INSTALL_PREFIX=/opt/ai-sw/llvm-mos -DCMAKE_PREFIX_PATH=/opt/ai-sw/llvm-mos \
      -DLLVM_MOS=/opt/ai-sw/llvm-mos -DLLVM_MOS_BOOTSTRAP_COMPILER=OFF \
 && cmake --build /build/llvm-mos-sdk-obj -j "${JOBS}" --target install \
 && /opt/ai-sw/llvm-mos/bin/mos-c64-clang --version

# rustc with i8086-unknown-none (llvm-i8086/rust/README.md). rust_build.sh fetches
# the pinned rust-lang/rust commit and writes /build/rust-obj (bootstrap.toml).
FROM llvm-i8086 AS rust-i8086
ARG JOBS=8
RUN --mount=type=cache,id=ai-sw-ccache-i8086-rust,target=/ccache \
    JOBS="${JOBS}" bash /src/llvm-i8086/scripts/build_llvm_for_rust.sh \
 && JOBS="${JOBS}" bash /src/llvm-i8086/scripts/rust_build.sh \
 && /build/rust-obj/x86_64-unknown-linux-gnu/stage1/bin/rustc --print target-list | grep -qx i8086-unknown-none

FROM base AS full
USER root
COPY --from=llvm-i4004 --chown=mos /src/llvm-i4004/install /opt/ai-sw/llvm-i4004/install
COPY --from=llvm-i8008 --chown=mos /src/llvm-i8008/install /opt/ai-sw/llvm-i8008/install
COPY --from=llvm-i8086 --chown=mos /src/llvm-i8086/install /opt/ai-sw/llvm-i8086/install
COPY --from=llvm-mos --chown=mos /opt/ai-sw/llvm-mos /opt/ai-sw/llvm-mos
# Keep the stage-1 sysroot at the path the scripts default to ($I8086_RUSTC), and the
# library sources its lib/rustlib/src/rust link points at, for -Zbuild-std=core.
COPY --from=rust-i8086 --chown=mos /build/rust-obj/x86_64-unknown-linux-gnu/stage1 /build/rust-obj/x86_64-unknown-linux-gnu/stage1
COPY --from=rust-i8086 --chown=mos /build/rust-src/library /build/rust-src/library
COPY --chown=mos docker/entrypoint.sh docker/verify_toolchains.sh /opt/ai-sw/bin/
# Point the sysroot's rust-src link at the copied library (bootstrap links it to the source tree).
RUN chmod 0755 /opt/ai-sw/bin/*.sh \
 && mkdir -p /build/rust-obj/x86_64-unknown-linux-gnu/stage1/lib/rustlib/src \
 && ln -sfn /build/rust-src /build/rust-obj/x86_64-unknown-linux-gnu/stage1/lib/rustlib/src/rust \
 && chown -h mos /build/rust-obj/x86_64-unknown-linux-gnu/stage1/lib/rustlib/src/rust
USER mos
ENV AI_SW_TOOLCHAINS=/opt/ai-sw \
    LLVM_MOS_HOME=/opt/ai-sw/llvm-mos \
    I8086_RUSTC=/build/rust-obj/x86_64-unknown-linux-gnu/stage1/bin/rustc \
    PATH=/opt/ai-sw/bin:${PATH}
# /bin/sh has no pipefail: keep the verifier's exit status, then record its report.
RUN /opt/ai-sw/bin/verify_toolchains.sh > /tmp/verify.txt || { cat /tmp/verify.txt; exit 1; }; \
    cat /tmp/verify.txt | tee -a /home/mos/TOOLCHAIN_VERSIONS.txt && rm /tmp/verify.txt
WORKDIR /workspace
ENTRYPOINT ["/opt/ai-sw/bin/entrypoint.sh"]
CMD ["bash"]

# Keep `docker build .` producing the light base image.
FROM base AS default
