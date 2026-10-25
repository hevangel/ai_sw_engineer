# Boot ROM oracle

The original EPROM's bytes are unpublished. The boot ROM's oracle is the
programs it loads: `scripts/run_sim.sh` boots the other titles' disk images
on the RTL and replays their own transcripts, which were captured from their
originals. The disk protocol follows the MO5 emulator (`../spec/spec.md`).
