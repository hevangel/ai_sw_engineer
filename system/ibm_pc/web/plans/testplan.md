# IBM PC 5150 browser frontend test plan

Expected values come from the hardware project's RTL (recorded transcripts in
`../emulator/oracle/`) and the original images' recorded hashes, never from this
build. The native emulator's own levels are in [../emulator/plans/testplan.md](../../emulator/plans/testplan.md).

| Level | What | Oracle | Command (from the repository root) |
|---|---|---|---|
| Binding units | media size checks with reasons, power-on without a BIOS, a stand-in ROM (not IBM's) runs and draws, disks mount/eject and keep writes across a power cycle, monitor breakpoints persist | hand-written stand-in program's own effect | `cargo +1.99.1 test --manifest-path system/ibm_pc/web/wasm/Cargo.toml` |
| WASM self-test | the built `.wasm` from Node: refusals with reasons, stand-in ROM to HLT, text rows, SHA-256, debugger | `node:crypto`; the stand-in program | `node system/ibm_pc/web/tests/node_replay.mjs` |
| WASM vs RTL session | the RTL's 20-step DOS and BASIC session (screens, video-memory hashes in graphics modes, drive B hash) through `pc_session` | `../emulator/oracle/transcripts/dos100_session.txt` | `IBM_PC_ROMS=DIR node system/ibm_pc/web/tests/node_replay.mjs` |
| WASM speed | one machine second of POST in batches of 1/100 s with audio and text reads | must be faster than real time | same command |
| Browser: errors and states | Power on disabled and the overlay naming the missing BIOS; wrong-size BIOS and disk: an alert and a status that name the required size; ready and paused overlays; state chip | spec "UI contract" | `IBM_PC_ROMS=DIR node system/ibm_pc/web/tests/browser.mjs` |
| Browser: media | the four originals recognised by hash; a generated blank disk not called original | `../emulator/original/provenance.md` | same |
| Browser: keyboard | DATE typed with real key events into the focused canvas, DIR, then CHKDSK through the text box: each screen; the capture badge; the Esc key button cancels a DOS line | the RTL's screens after steps 1-4; DOS's own `\` line cancel | same |
| Browser: media convenience | a reload restores the remembered BIOS, Forget clears it; dropped BIOS and BASIC files land in their slots by hash | W-A4; provenance hashes | same |
| Browser: compiled software | the `llvm-i8086` Rust `hello_rs` floppy (exported by `boot_oracle`) booted from drive A | `../emulator/oracle/boot/hello_rs.txt` (screen and image SHA-256) | same, with `hello_rs.img` in `DIR` |
| Browser: media writes | COPY to a generated blank disk B, download it, and again after a power cycle | the directory entry `CLOCK   COM`; W-A1 | same |
| Browser: execution | pause stops the clock, resume, Ctrl+Alt+Del warm boot, speed "max" then real time (status 90-110%), sound samples reach Web Audio | spec "Execution model" | same |
| Browser: responsiveness | no main-thread task over 100 ms in 2 s of real-time running; 20-65 frames per second | spec | same |
| Browser: SV mode | the unavailable panel and its reason; switching back leaves the machine off | spec "Mode B" | same |
| Browser: layout and hygiene | no horizontal scroll at 390 px; no console errors; no request outside the page's origin; Ctrl+] releases focus; light, dark and narrow screenshots | spec | same, `--shots DIR` saves screenshots |

Not automated: listening to the sound, real keyboards with non-US layouts, browsers
other than Chromium, very slow hosts.
