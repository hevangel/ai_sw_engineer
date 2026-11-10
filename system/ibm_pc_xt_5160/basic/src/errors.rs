//! Error numbers and messages (spec §12): 1-30 and 50-67 are the ROM's (03B5h), 68-76
//! BASIC.COM's; every other number prints `Unprintable error`.

pub const NEXT_WITHOUT_FOR: u8 = 1;
pub const SYNTAX: u8 = 2;
pub const ILLEGAL_FUNCTION_CALL: u8 = 5;
pub const OVERFLOW: u8 = 6;
pub const OUT_OF_MEMORY: u8 = 7;
pub const UNDEFINED_LINE: u8 = 8;
pub const TYPE_MISMATCH: u8 = 13;
pub const STRING_TOO_LONG: u8 = 15;
pub const CANT_CONTINUE: u8 = 17;
pub const LINE_BUFFER_OVERFLOW: u8 = 23;
pub const ADVANCED_FEATURE: u8 = 73;

static LOW: &[&[u8]] = &[
    b"NEXT without FOR",
    b"Syntax error",
    b"RETURN without GOSUB",
    b"Out of DATA",
    b"Illegal function call",
    b"Overflow",
    b"Out of memory",
    b"Undefined line number",
    b"Subscript out of range",
    b"Duplicate Definition",
    b"Division by zero",
    b"Illegal direct",
    b"Type mismatch",
    b"Out of string space",
    b"String too long",
    b"String formula too complex",
    b"Can't continue",
    b"Undefined user function",
    b"No RESUME",
    b"RESUME without error",
    b"",
    b"Missing operand",
    b"Line buffer overflow",
    b"Device Timeout",
    b"Device Fault",
    b"FOR Without NEXT",
    b"Out of Paper",
    b"",
    b"WHILE without WEND",
    b"WEND without WHILE",
];

static HIGH: &[&[u8]] = &[
    b"FIELD overflow",
    b"Internal error",
    b"Bad file number",
    b"File not found",
    b"Bad file mode",
    b"File already open",
    b"",
    b"Device I/O Error",
    b"File already exists",
    b"",
    b"",
    b"Disk full",
    b"Input past end",
    b"Bad record number",
    b"Bad file name",
    b"",
    b"Direct statement in file",
    b"Too many files",
    b"Device Unavailable",
    b"Communication buffer overflow",
    b"Disk Write Protected",
    b"Disk not Ready",
    b"Disk Media Error",
    b"Advanced Feature",
    b"Rename across disks",
    b"Path/File access error",
    b"Path not found",
];

/// The message of error `n`.
pub fn message(n: u8) -> &'static [u8] {
    let m = match n {
        1..=30 => LOW[n as usize - 1],
        50..=76 => HIGH[n as usize - 50],
        _ => b"",
    };
    if m.is_empty() {
        b"Unprintable error"
    } else {
        m
    }
}
