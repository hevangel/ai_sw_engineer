# basic test plan

| Level | What | Oracle | Command |
|---|---|---|---|
| Host unit | core routines | manual / listing | `cargo test` |
| Host replay | all transcripts | original on simulator | `cargo test --test oracle_replay` |
| Target | image on primary simulator | same transcripts | `scripts/run_sim.sh` + `scripts/compare.sh` |
| Cross-check | image on secondary emulator | same transcripts | `SIM=secondary scripts/run_sim.sh` |

Coverage goal: every feature in `spec/spec.md` appears in at least one transcript.
