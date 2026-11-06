// Host keyboard -> IBM PC 83-key keyboard, scan code set 1 (make codes;
// the break code is make | 0x80). Keyed by KeyboardEvent.code, which names
// the physical key, so the layout follows the US 5150 keyboard regardless of
// the host's language setting. Source: IBM Technical Reference (August 1981),
// "Keyboard", scan code table.
//
// The 5150 has no separate cursor block: arrows, Home/End, PgUp/PgDn, Ins and
// Del are the numeric-pad keys (with Num Lock off). Right Ctrl/Alt map to the
// only Ctrl/Alt. Keys the 5150 lacks (F11, F12, Windows, menu) are not mapped.
export const SCAN = Object.freeze({
  Escape: 0x01,
  Digit1: 0x02, Digit2: 0x03, Digit3: 0x04, Digit4: 0x05, Digit5: 0x06,
  Digit6: 0x07, Digit7: 0x08, Digit8: 0x09, Digit9: 0x0A, Digit0: 0x0B,
  Minus: 0x0C, Equal: 0x0D, Backspace: 0x0E, Tab: 0x0F,
  KeyQ: 0x10, KeyW: 0x11, KeyE: 0x12, KeyR: 0x13, KeyT: 0x14,
  KeyY: 0x15, KeyU: 0x16, KeyI: 0x17, KeyO: 0x18, KeyP: 0x19,
  BracketLeft: 0x1A, BracketRight: 0x1B, Enter: 0x1C,
  ControlLeft: 0x1D, ControlRight: 0x1D,
  KeyA: 0x1E, KeyS: 0x1F, KeyD: 0x20, KeyF: 0x21, KeyG: 0x22,
  KeyH: 0x23, KeyJ: 0x24, KeyK: 0x25, KeyL: 0x26,
  Semicolon: 0x27, Quote: 0x28, Backquote: 0x29,
  ShiftLeft: 0x2A, Backslash: 0x2B, IntlBackslash: 0x2B,
  KeyZ: 0x2C, KeyX: 0x2D, KeyC: 0x2E, KeyV: 0x2F, KeyB: 0x30,
  KeyN: 0x31, KeyM: 0x32, Comma: 0x33, Period: 0x34, Slash: 0x35,
  ShiftRight: 0x36, NumpadMultiply: 0x37, PrintScreen: 0x37,
  AltLeft: 0x38, AltRight: 0x38, Space: 0x39, CapsLock: 0x3A,
  F1: 0x3B, F2: 0x3C, F3: 0x3D, F4: 0x3E, F5: 0x3F,
  F6: 0x40, F7: 0x41, F8: 0x42, F9: 0x43, F10: 0x44,
  NumLock: 0x45, ScrollLock: 0x46,
  Numpad7: 0x47, Home: 0x47, Numpad8: 0x48, ArrowUp: 0x48,
  Numpad9: 0x49, PageUp: 0x49, NumpadSubtract: 0x4A,
  Numpad4: 0x4B, ArrowLeft: 0x4B, Numpad5: 0x4C,
  Numpad6: 0x4D, ArrowRight: 0x4D, NumpadAdd: 0x4E,
  Numpad1: 0x4F, End: 0x4F, Numpad2: 0x50, ArrowDown: 0x50,
  Numpad3: 0x51, PageDown: 0x51, Numpad0: 0x52, Insert: 0x52,
  NumpadDecimal: 0x53, Delete: 0x53,
  NumpadEnter: 0x1C, NumpadDivide: 0x35,
});

// The key combination that gives keyboard focus back to the page while the
// screen is capturing keys (Tab and Escape belong to the PC).
export function isReleaseCombo(e) {
  return e.ctrlKey && e.code === 'BracketRight';
}

// Ctrl+Alt+Del cannot be typed into a browser on every OS; the UI has a
// button that sends this sequence instead.
export const CTRL_ALT_DEL = [0x1D, 0x38, 0x53, 0xD3, 0xB8, 0x9D];
