# IBM PC 5150 browser frontend — usability review and polish plan (issue #250)

Review of the functional baseline (issue #249) used as a first-time
user would: load the originals, boot DOS, type, use a disk, debug, try the SV mode, on a
1280 px desktop and a 390 px phone. Screenshots of that baseline
(`report/screenshots/functional/`, not committed) come from `tests/browser.mjs --shots`
run at the functional-stage commit. Each finding has
the change that answers it; the "after" evidence is in
[../report/verification.md](../report/verification.md).

## Findings

| # | Finding (baseline) | Severity | Change |
|---|---|---|---|
| U1 | The screen, the reason the page exists, sits below a long media table: on a phone it is three screens down; on a desktop the table pushes it below the fold | high | Two columns on wide screens (monitor left, media and tools right); on narrow screens the monitor comes first |
| U2 | Before power-on the screen is a black box with no explanation; after a backend switch it keeps showing the last frame while the status says "Machine off" | high | An overlay on the screen for every non-running state: what is missing (BIOS), "Power on", "Paused", "Off" |
| U3 | "Power on" is enabled with no BIOS and only then reports an error | medium | Power is the one primary button and stays disabled until a BIOS is loaded; the overlay and a hint say why |
| U4 | Nothing shows whether keys go to the PC; the Ctrl+] release is only in a paragraph | high | A keyboard-capture badge on the monitor ("Keyboard → PC · Ctrl+] to release" / "Click to type") |
| U5 | Five raw file inputs; SHA-256 prefixes are noise to most users; drive activity is invisible | medium | Media cards: name, size, an "original ✓" or "user image" badge (full hash in a tooltip and on request), drive motor lights; drag-and-drop of files anywhere on the page, assigned by recorded hash or size |
| U6 | ROMs must be chosen again on every visit | medium | "Remember in this browser" (on by default; IndexedDB, local only, a Forget button); failures fall back silently to the old behaviour |
| U7 | Phones have no Esc, Tab, function keys or Ctrl+Break | medium | A special-key row (Esc, Tab, Enter, Bksp, arrows, F1-F10, Ctrl+Break, Ctrl+Alt+Del) |
| U8 | Screen text is not available to screen readers or for copying | medium | "Copy screen text" and a "Show as text" toggle (a `pre` with the 25 rows, refreshed on demand) |
| U9 | Debugger commands while running change state under the user; registers scroll in a plain box | low | Debugger commands pause first (said in the panel); monospace output with a clear button |
| U10 | The backend select hides that Mode B is unavailable until chosen | low | A two-option segmented control; the SV option says "unavailable" in its label and explains on selection |
| U11 | Plain browser styling: weak hierarchy, no sense of the machine | low | A monitor bezel around the screen, consistent spacing scale, colour tokens with light and dark themes, focus rings kept |
| U12 | Status text mixes state, time and speed in one sentence | low | A status bar: state chip, machine time, speed %, drive lights |

## Constraints kept

- Real machine output only: the canvas shows the emulator's frame; overlays sit on top
  and disappear while running. No mock-ups.
- The functional contract (`../spec/spec.md`) is unchanged: same worker protocol, WASM
  ABI, error texts, keyboard mapping, Mode B state. No backend behaviour added for looks.
- `tests/browser.mjs` keeps every functional check; selectors change only where the
  control changed.
