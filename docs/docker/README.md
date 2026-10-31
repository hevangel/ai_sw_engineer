# Docker toolchain image

`Dockerfile` at the repo root builds `ai-sw-engineer:latest`:

- base: `mrkits/rust-mos:<pinned tag>` (Ubuntu 22.04, llvm-mos + llvm-mos-sdk in `/usr/local`, rust-mos linked as rustup toolchain `mos`, `RUST_TARGET_PATH=/usr/local/rust-mos/targets`)
- host Rust: pinned stable (default toolchain) with rustfmt/clippy
- pinned nightly with `rust-src` + `llvm-tools` for `-Zbuild-std=core`
- clang, lld, cmake, ninja, ccache, python3, `lit` — for building this repo's own backends

The image holds tools only. The repo is bind-mounted at `/workspace`; `ai_hw_engineer` at `/ai_hw_engineer`. Backends from `llvm-<chip>/` are built inside the container into `llvm-<chip>/install/` (gitignored) and persist on the host.

```bash
docker build -t ai-sw-engineer .
./scripts/docker_run.sh                       # interactive shell
./scripts/docker_run.sh cargo test --manifest-path system/<sys>/<sw>/Cargo.toml
cat /home/mos/TOOLCHAIN_VERSIONS.txt          # inside: exact versions for reports
```

Pins (change together, record in the PR): `RUST_MOS_IMAGE`, `RUST_STABLE`, `RUST_NIGHTLY` build args.

On Windows run `scripts/docker_run.sh` from Git Bash or WSL; from PowerShell use `docker run --rm -it -v "${PWD}:/workspace" ai-sw-engineer`.

## Full toolchain image (`docker compose`)

`docker-compose.yml` builds the Dockerfile's `full` target, `ai-sw-engineer:full`: the base image above plus every toolchain this repository builds itself, compiled once into the image instead of into the `ai-sw-llvm-build` volume.

| Toolchain | Built by | In the image |
|---|---|---|
| 6502 family: llvm-mos, llvm-mos-sdk, rust-mos | upstream image (base stage) | `/usr/local`, `rustc +mos` |
| M68k: pinned nightly `m68k-unknown-none-elf`, GNU m68k binutils | base stage | `rustc +nightly-…`, `m68k-linux-gnu-ld` |
| `llvm-i4004`, `llvm-i8008`, `llvm-i8086` (llc, llvm-mc, llvm-objdump, FileCheck, …) | each backend's `scripts/build_llvm.sh`, one BuildKit stage per backend | `/opt/ai-sw/llvm-<chip>/install` |
| rustc with `i8086-unknown-none` (stage 1, `core`/`alloc`/`compiler_builtins`) | `llvm-i8086/scripts/build_llvm_for_rust.sh` + `rust_build.sh` | `/build/rust-obj/x86_64-unknown-linux-gnu/stage1` (`$I8086_RUSTC`), library sources in `/build/rust-src/library` |

`llvm-i286`, `llvm-m6800` and `llvm-m6809` are audit-only and have nothing to build yet; `llvm-mos6502` uses the upstream llvm-mos.

```bash
scripts/compose_build.sh                        # = docker compose build toolchain, LLVM pinned to the submodule
JOBS=16 scripts/compose_build.sh                # parallelism of each LLVM/rustc build (default 8)
docker compose run --rm toolchain               # interactive shell, repo at /workspace, ai_hw_engineer at /ai_hw_engineer
docker compose run --rm toolchain llvm-i8086/scripts/run_tests.sh
docker compose run --rm toolchain verify_toolchains.sh   # one line per toolchain; nonzero if one is missing
docker compose --profile base build base        # the light image only (same as `docker build .`)
```

- **LLVM pin.** The build fetches exactly `LLVM_PROJECT_COMMIT` from github.com/llvm/llvm-project, so the 2 GB submodule is not part of the build context. `scripts/compose_build.sh` reads the commit from `git ls-tree HEAD external/llvm-project` and refuses a different value; keep the default in `docker-compose.yml` equal to the pin when the submodule moves. The rustc source pin lives in `llvm-i8086/scripts/rust_build.sh`.
- **Where scripts find the tools.** The backend scripts use `<repo>/llvm-<chip>/install/bin`. When the mounted repository has no `install/` of its own, the entrypoint links it to the image's copy (`llvm-*/install` is gitignored); a locally built `install/` always wins. Set `AI_SW_LINK_TOOLCHAINS=0` to disable the link. The i8086 scripts default `$I8086_RUSTC` to the path the image provides, so do not mount the old `ai-sw-llvm-build` volume over `/build` in this image.
- **Cost.** The first build compiles three MC/CodeGen LLVMs, the LLVM libraries for rustc and rustc itself: roughly an hour on 16 cores and tens of GB of BuildKit cache; the backend stages run in parallel and ccache is kept in BuildKit cache mounts. Requires BuildKit (Docker 23+ or `docker compose`).
- **Check.** The `full` stage ends with `docker/verify_toolchains.sh`, so an image without a working i4004/i8008/i8086 LLVM, the i8086 rustc and its library sources, rust-mos or the M68k route fails to build. Its output is appended to `/home/mos/TOOLCHAIN_VERSIONS.txt`.

