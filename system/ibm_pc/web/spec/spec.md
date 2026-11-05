# IBM PC 5150 browser frontend — specification and UI/backend contract

Owning issues: hevangel/ai_sw_engineer#88 (parent), #249 (functional stage, this
document's first version), #250 (UI/UX polish). Mode A is delivered here; Mode B
(SystemVerilog through Xezim) is a selector with an explanatory unavailable state,
not an implementation (see "Mode B").

The execution engine of Mode A is the native Rust system emulator
([../emulator](../emulator/README.md)) compiled to WebAssembly: the same `Machine`
that replays the hardware project's RTL session exactly. There is no second,
browser-only machine model; `wasm/src/lib.rs` only moves bytes across the boundary.

## Machine profile

| Aspect | Value | Source |
|---|---|---|
| Board | IBM 5150, SW1 = 6Fh: two drives, 80x25 colour, no 8087, 64 KiB | emulator spec E-A12 |
| CPU | 8088 at 4.772727 MHz, instruction-level timing | emulator spec E-A2 |
| ROM | 8 KiB BIOS (required) at FE000h, 32 KiB Cassette BASIC (optional) at F6000h | `Machine::load_firmware` |
| Video | CGA: 40/80x25 text with the character ROM, 320x200x4 and 640x200x2 graphics | emulator spec, "Frames" |
| Media | drives A and B, single-sided 160 KiB images (40 x 8 x 512) | emulator spec E-A10 |
| Sound | speaker (timer 2 AND port B bit 1) | emulator spec E-A13 |
| Keyboard | 83-key, scan code set 1 | `www/js/keys.js` |

Not available in the browser: cassette, printer, serial, game port, more than 64 KiB,
cycle-exact timing (all also outside the native emulator's contract).

## Mode A — default Rust/WebAssembly machine

### Execution model

- Static files only (`www/`): no server code, no remote simulator, no install. Any
  static server works; `file://` does not (the worker and the WASM are fetched).
- The WASM module runs in a dedicated Web worker (`www/js/worker.js`). The UI thread
  only draws frames, plays sound and forwards input.
- **Bounded batches.** Each worker tick runs the machine for the wall time since the
  previous tick, at most 50 ms of machine time (`MAX_BATCH_MS`), then yields for 4 ms
  so queued messages (keys, pause) are handled. "As fast as possible" runs whole
  batches of 1/200 s of machine time for up to 12 ms of wall time per tick; sound is
  not sent in that mode.
- **Real time** is 4,772,727 machine clocks per wall second; the status line reports
  the measured ratio. If the host is too slow the machine runs slower, it never skips.
- **Frames** are sent at most every 1/60 s (the CGA refresh) and only after the page
  acknowledged the previous one (`frameAck`), so a slow page is never flooded.
- **Determinism.** The machine is the native crate's: the same media and the same key
  sequence at the same machine clocks give the same screens. Interactive keys arrive
  at host-dependent clocks, so an interactive session is not bit-replayable; the
  deterministic replay is `pc_session` (the native transcript format, used by
  `tests/node_replay.mjs`).
- **Restart** is power-on: the machine is rebuilt from the loaded ROMs, RAM cleared;
  the diskettes keep what the machine wrote to them (as real floppies do). Loading a
  file into a drive replaces its image. Ctrl+Alt+Del is the BIOS's warm boot.
- No persistence across page loads and no snapshot format.

### WASM ABI

Crate `wasm/` (`ibm_pc-emulator-wasm`, cdylib, depends only on `../emulator`; raw C
ABI, no wasm-bindgen). `ERR` is `u32::MAX` (-1 in JS before `>>> 0`); its message is
in the output buffer. The module owns all buffers; the host re-reads
`memory.buffer` after every call.

| Export | Contract |
|---|---|
| `pc_scratch(len) -> *mut u8` | input buffer of `len` bytes |
| `pc_load(slot, len, write_protect) -> u32` | slot 0 BIOS (8,192 B), 1 BASIC (32,768 B), 2 character ROM (8,192 B IBM or 2,048 B 8x8 font), 3 drive A, 4 drive B (163,840 B). 0 or ERR with the expected size. Disks also go into a running machine; ROMs take effect at the next power-on, the character ROM at once |
| `pc_eject(drive)` | empty drive 0/1 (stays empty after reset) |
| `pc_power_on(audio) -> u32` | build and reset the machine; disks keep the machine's writes; ERR without a BIOS |
| `pc_set_audio(on)` | speaker capture on/off |
| `pc_run(clocks) -> u32` | one batch; 0 ran, 1 CPU halted (waiting for an interrupt), 2 no machine, ERR undefined opcode (message with registers) |
| `pc_key(code)`, `pc_type(len) -> u32` | queue one scan code / type UTF-8 text (US layout, `\r` Enter); returns codes queued |
| `pc_keys_pending() -> u32` | scan codes not yet read by the machine |
| `pc_frame() -> u32`, `pc_frame_ptr()` | RGBA frame, `width << 16 \| height`; 0 in a text mode without a character ROM |
| `pc_text() -> u32` | the 25 text rows, `\n`-joined, in the output buffer |
| `pc_video_state() -> u32` | CGA mode register, colour select << 8, cursor address << 16 |
| `pc_audio(rate) -> u32`, `pc_audio_ptr()` | speaker samples (f32, -1..1) since the last call |
| `pc_clock() -> f64`, `pc_instructions() -> f64` | machine clocks, instructions since reset |
| `pc_drive_status() -> u32` | diskette DOR: bits 4-7 drive motors |
| `pc_disk(drive) -> u32` | the current image of a drive (with the machine's writes) in the output buffer |
| `pc_sha256(len) -> u32` | SHA-256 hex of the scratch bytes |
| `pc_monitor(len) -> u32` | one native `--monitor` command; breakpoints persist until power-on |
| `pc_session(len) -> u32` | replay a native session script on a fresh power-on; transcript out |
| `pc_out_ptr()`, `pc_out_len()`, `pc_build_id()` | output buffer; NUL-terminated build id |

### Worker messages

Page → worker: `init`, `load {slot, bytes, writeProtect, name}`, `eject {drive}`,
`power {audio}`, `off`, `pause`, `resume`, `speed {mode: real|max}`,
`audio {on, rate}`, `key {code}`, `keys {codes}`, `type {text}`, `frameAck`,
`monitor {line}`, `disk {drive}`, `status`.
Worker → page: `ready {build}`, `loaded {slot, name, size, sha256, ok, error}`,
`ejected`, `powered`, `frame {width, height, pixels} | {width: 0, text, video}`,
`audio {samples, rate}`, `status {running, powered, clock, instructions, speedRatio,
drives, keysPending}`, `typed {count}`, `monitor {line, text}`, `disk {drive, bytes}`,
`error {message}`.

### UI contract

- **Media.** File inputs only; nothing is bundled, fetched or uploaded. Each file's
  size and SHA-256 are shown, and it is labelled `original: <name>` only when the hash
  equals a recorded original ([provenance](../emulator/original/provenance.md)),
  else `user-provided image`. Drive A is write-protected by default (the original
  system disk is never modified). "Insert blank disk" generates a writable data disk
  (W-A1). "Download disk B" saves drive B's current image with the machine's writes.
- **Screen.** A focusable canvas (640 or 320 x 200, shown at 4:3). With the
  character ROM, frames come from `Cga::frame` (colours, cursor, blink, graphics
  modes). Without it, text rows are drawn with a browser monospace font, light grey,
  without attributes, and the media table says so (W-A2). Graphics modes need no ROM.
- **Keyboard.** While the canvas has focus, every mapped key goes to the PC
  (`keys.js`, by `KeyboardEvent.code`: physical position, US 5150 layout), make on
  keydown (repeating while held, like the 5150's typematic), break on keyup, all
  held keys released on blur. Ctrl+] gives focus back to the page. A text box types
  strings through `pc_type`; a button sends Ctrl+Alt+Del.
- **Controls.** Power on / Restart (power cycle), Pause/Resume, speed, sound (Web Audio
  starts only after the user ticks it), debugger panel (the native monitor's
  commands). Errors appear in an `alert` region; the status line is `aria-live`.
- **Sound.** Samples are DC-blocked (the speaker's idle level is constant) and
  scheduled 50 ms ahead; a gap resynchronises instead of drifting.

## Mode B — optional Xezim/SystemVerilog machine (unavailable)

The backend selector offers "SystemVerilog machine (Xezim)". Selecting it powers the
Rust machine off (a backend switch resets; there is no cross-backend state transfer)
and shows a panel that says the mode is unavailable and why: the qualified HDL
Workbench/Xezim adapter (hevangel/ai_engineer#55) and the selected SV system
(hevangel/ai_engineer#80) are not available to this page. No signals, waveforms or
speed figures are shown, and the Rust model is never presented as SV simulation.
"Back to the Rust emulator" restores Mode A, powered off. The full contract of #88
for Mode B (same controls and I/O, signal selection, waveform capture/export, Surfer,
normalized trace time) is open work.

## Assumptions

| ID | Assumption | Why | How it is validated |
|---|---|---|---|
| W-A1 | A data disk with both FATs starting FE FF FF, the 64 directory entries starting E5h, the rest of the system area zero and the data area F6h is writable by PC DOS 1.00 | PC DOS 1.00 counts directory entries that start with 00h as files (`../chkdsk/spec/spec.md`), so a zero-filled directory is "full" | `tests/browser.mjs`: `COPY TIME.COM B:CLOCK.COM` succeeds and the downloaded image's directory holds `CLOCK   COM`. Not byte-identical to what FORMAT writes |
| W-A2 | Without the character ROM, text is drawn in a browser font without colours, cursor or blink | the ROM is IBM's and is not shipped | the media table says so; the frame path with the ROM is the checked one |
| W-A3 | Host key repeat stands in for the 5150 keyboard's typematic repeat | the browser repeats keydown at the host's rate | interactive only; the deterministic tests type with explicit presses |
