# Busicom 141-PF platform

- [memory_map.md](memory_map.md) — ROM/RAM spaces and the 1280-byte budget
- [io_map.md](io_map.md) — ports, keyboard matrix, printer hammer word
- [link.ld](link.ld) — placeholder until `llvm-i4004` defines its object format
- `runtime/` — reset code and the port-level drivers (keyboard scan via 4003, printer drum sequencing on TEST) — milestone M3
- `scripts/run_image.sh` — will load an image as the ROM of the ai_hw_engineer board (primary) or `external/busicom` (secondary)
