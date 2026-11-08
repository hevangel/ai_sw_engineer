//! Host text to IBM PC keyboard scan codes (set 1: make code, break = make | 80h).

const LETTER_CODES: [u8; 26] = [
    30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45,
    21, 44,
];
const DIGIT_CODES: [u8; 10] = [11, 2, 3, 4, 5, 6, 7, 8, 9, 10];
const LEFT_SHIFT: u8 = 42;
const LEFT_CTRL: u8 = 29;

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
        '\\' => 43,
        '`' => 41,
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
        '<' => Some(','),
        '>' => Some('.'),
        '|' => Some('\\'),
        '~' => Some('`'),
        'A'..='Z' => Some(c.to_ascii_lowercase()),
        _ => None,
    };
    match shifted_base {
        Some(b) => base(b).map(|k| (k, true)),
        None => base(c).map(|k| (k, false)),
    }
}

/// Scan codes that type `text` on a US keyboard, shift handled around each
/// shifted character and Ctrl around each control character 01h-1Ah (Ctrl with
/// that letter; `\r` stays Enter). Characters without a key are skipped.
pub fn scan_codes(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for c in text.chars() {
        if ('\u{1}'..='\u{1a}').contains(&c) && c != '\r' {
            let letter = (b'a' + c as u8 - 1) as char;
            if let Some((key, _)) = key_for(letter) {
                out.extend([LEFT_CTRL, key, key | 0x80, LEFT_CTRL | 0x80]);
            }
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::scan_codes;

    #[test]
    fn control_characters_are_typed_with_ctrl() {
        // ^Z: Ctrl down, Z, Z up, Ctrl up; Enter stays Enter
        assert_eq!(scan_codes("\u{1a}"), vec![29, 44, 44 | 0x80, 29 | 0x80]);
        assert_eq!(scan_codes("\u{3}"), vec![29, 46, 46 | 0x80, 29 | 0x80]);
        assert_eq!(scan_codes("\r"), vec![28, 28 | 0x80]);
    }

    #[test]
    fn redirection_and_pipe_characters_have_keys() {
        // < > | are Shift with , . and \ (scan codes 33h, 34h, 2Bh) on the US keyboard.
        assert_eq!(scan_codes("<"), vec![42, 51, 51 | 0x80, 42 | 0x80]);
        assert_eq!(scan_codes(">"), vec![42, 52, 52 | 0x80, 42 | 0x80]);
        assert_eq!(scan_codes("|"), vec![42, 43, 43 | 0x80, 42 | 0x80]);
        assert_eq!(scan_codes("\\"), vec![43, 43 | 0x80]);
    }
}
