// Busicom 141-PF keyboard: the 30 populated matrix keys in the physical
// layout (platform/io_map.md). Code = 129 + 4*row + column.
'use strict';

export const KEYPAD_ROWS = [
  [
    { label: 'CM', code: 129, key: 'q', title: 'Clear memory (Q)' },
    { label: 'RM', code: 130, key: 'w', title: 'Recall memory (W)' },
    { label: 'M−', code: 131, key: 'a', title: 'Memory subtract (A)' },
    { label: 'M+', code: 132, key: 's', title: 'Memory add (S)' },
  ],
  [
    { label: '√', code: 133, key: 'd', title: 'Square root (D)' },
    { label: '%', code: 134, key: 'f', title: 'Percent (F)' },
    { label: 'M=−', code: 135, key: 'z', title: 'Memory equals minus (Z)' },
    { label: 'M=+', code: 136, key: 'x', title: 'Memory equals plus (X)' },
  ],
  [
    { label: '◇', code: 137, key: 't', title: 'Subtotal (T)' },
    { label: '÷', code: 138, key: '/', title: 'Divide (/)' },
    { label: '×', code: 139, key: '*', title: 'Multiply (*)' },
    { label: '=', code: 140, key: 'Enter', title: 'Equals (Enter or =)' },
  ],
  [
    { label: '−', code: 141, key: '-', title: 'Minus (−)' },
    { label: '+', code: 142, key: '+', title: 'Plus (+)' },
  ],
  [
    { label: '9', code: 145, key: '9' },
    { label: '6', code: 146, key: '6' },
    { label: '3', code: 147, key: '3' },
    { label: '.', code: 148, key: '.', title: 'Decimal point (. or ,)' },
  ],
  [
    { label: '8', code: 149, key: '8' },
    { label: '5', code: 150, key: '5' },
    { label: '2', code: 151, key: '2' },
    { label: '00', code: 152, key: 'j', title: 'Double zero (J)' },
  ],
  [
    { label: '7', code: 153, key: '7' },
    { label: '4', code: 154, key: '4' },
    { label: '1', code: 155, key: '1' },
    { label: '0', code: 156, key: '0' },
  ],
  [
    { label: 'SIGN', code: 157, key: 'n', title: 'Sign change (N)' },
    { label: 'EX', code: 158, key: 'e', title: 'Exchange (E)' },
    { label: 'CE', code: 159, key: 'Backspace', title: 'Clear entry (Backspace)' },
    { label: 'C', code: 160, key: 'Escape', title: 'Clear (Escape or Delete)' },
  ],
];

// PC keyboard character -> matrix code (derived from KEYPAD_ROWS).
export const KEY_MAP = (() => {
  const map = new Map();
  for (const row of KEYPAD_ROWS) {
    for (const k of row) {
      if (k.key) map.set(k.key.toLowerCase(), k.code);
    }
  }
  // extra aliases
  map.set('=', 140); // equals
  map.set(',', 148); // point
  map.set('delete', 160); // clear
  return map;
})();
