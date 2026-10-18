# Tool documentation

One subfolder per tool, written for an AI agent: what the tool does here, the exact commands this repo uses, and the pitfalls hit so far.

| Folder | Tool |
|---|---|
| [docker/](docker/README.md) | the `ai-sw-engineer` toolchain image |
| [rust-bare-metal/](rust-bare-metal/README.md) | `no_std` crates, custom target JSON, `-Zbuild-std`, emitting LLVM IR |
| [llvm-mos/](llvm-mos/README.md) | llvm-mos, llvm-mos-sdk and rust-mos for the 6502 family |
| [llvm-backend/](llvm-backend/README.md) | writing a new LLVM target and linking rustc to it |
