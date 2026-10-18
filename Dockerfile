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

ARG RUST_MOS_IMAGE=mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8
FROM ${RUST_MOS_IMAGE}

ARG RUST_MOS_IMAGE
ARG RUST_STABLE=1.99.1
ARG RUST_NIGHTLY=nightly-2026-10-15

USER root
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl git git-lfs \
      clang lld cmake ninja-build ccache \
      python3 python3-pip python3-venv \
      xxd file less zstd \
 && rm -rf /var/lib/apt/lists/*

# lit + FileCheck driver for backend tests (FileCheck itself comes from LLVM builds).
RUN python3 -m pip install --no-cache-dir lit==18.1.8

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
