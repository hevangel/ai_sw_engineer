# Boot ROM test plan

| Level | What | Oracle | Command | Image |
|---|---|---|---|---|
| Host | seek, sector order, checksum, retry on a bad sector, against a disk and stack-card model | the MO5 emulator's protocol (spec) | `cargo test --release` | ai-sw-engineer |
| Target | the ROM boots the other titles' disk images on the RTL; their transcripts must match | the titles' oracles | `scripts/run_sim.sh` | ai-hw-engineer |
| Cross-check | MO5's Virtual Micral N | — | not done (not in `external/`) | — |

The original EPROM's bytes are not available, so there is no byte oracle
for the ROM itself: a boot is correct when the loaded program behaves as
its own original.
