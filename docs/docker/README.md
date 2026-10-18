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
