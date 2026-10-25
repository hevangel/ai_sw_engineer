# Shooting Stars test plan

| Level | What | Oracle | Command | Image |
|---|---|---|---|---|
| Host replay | all transcripts through the Rust game and a mock teletype | the original on the RTL | `cargo test --release` | ai-sw-engineer |
| Target, RAM | `build/shooting_stars.bin` at 0x0000 on the RTL | same | `scripts/run_sim.sh ram` | ai-hw-engineer |
| Target, boot | `build/disk.img` loaded by the Rust boot ROM | same | `scripts/run_sim.sh boot` | ai-hw-engineer |
| Cross-check | a second emulator | — | not done | — |

| Transcript | Covers |
|---|---|
| 01_rules_and_win | the rules pages, the shortest win (11 shots), no replay |
| 02_no_rules_lose | a shortest loss |
| 03_bad_shots | invalid keys, a black hole |
| 04_play_again | lose, play again, win |

Not covered: DEL at "YOUR SHOT?" (SS-1).
