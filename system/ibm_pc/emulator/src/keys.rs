//! Host text to IBM PC keyboard scan codes (set 1: make code, break = make | 80h).

const LETTER_CODES: [u8; 26] = [
    30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45,
    21, 44,
];
const DIGIT_CODES: [u8; 10] = [11, 2, 3, 4, 5, 6, 7, 8, 9, 10];
const LEFT_SHIFT: u8 = 42;

/// Scan code of an unshifted character, if it has a key.
fn base(c: char) -> Option<u8> {
    Some(match c {
        'a'..='z' => LETTER_CODES[c as usize - 'a' as usize],
        '0'..='9' => DIGIT_CODES[c as usize - '0' as usize],
        ' ' => 57,
        '\r' | '\n' => 28,
        '-' => 12,
        '=' => 13,
        ',' => 51,
        '.' => 52,
        '/' => 53,
        ';' => 39,
        '\'' => 40,
        '[' => 26,
        ']' => 27,
        _ => return None,
    })
}

/// The unshifted key and shift state that types `c`.
fn key_for(c: char) -> Option<(u8, bool)> {
    let shifted_base = match c {
        ':' => Some(';'),
        '"' => Some('\''),
        '*' => Some('8'),
        '(' => Some('9'),
        ')' => Some('0'),
        '+' => Some('='),
        '?' => Some('/'),
        '!' => Some('1'),
        '_' => Some('-'),
        'A'..='Z' => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    match shifted_base {
        Some(b) => base(b).map(|k| (k, true)),
        None => base(c).map(|k| (k, false)),
    }
}

/// Scan codes that type `text` on a US keyboard, shift handled around each
/// shifted character. Characters without a key are skipped.
pub fn scan_codes(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for c in text.chars() {
        if let Some((key, shifted)) = key_for(c) {
            if shifted {
                out.push(LEFT_SHIFT);
            }
            out.extend([key, key | 0x80]);
            if shifted {
                out.push(LEFT_SHIFT | 0x80);
            }
        }
    }
    out
}
