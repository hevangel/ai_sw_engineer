//! Shooting Stars (Willard I. Nico, BYTE, May 1976), reimplemented in Rust
//! (spec/spec.md). The universe is nine positions: eight fringe stars, one
//! bit each in `fringe`, and the centre (position 5) in `centre`. Shooting a
//! star flips its galaxy (MASKS). The output is the original's byte for byte,
//! including its two-CR line breaks.
#![no_std]

pub mod hal;

pub use hal::Hal;

const CR: u8 = 0x0D;
const LF: u8 = 0x0A;
const DEL: u8 = 0x7F;

/// Per star key '1'..'9': the fringe bit it occupies (0 for the centre),
/// the fringe stars its galaxy flips, and whether it flips the centre. The
/// original's MASKTAB.
const MASKS: [(u8, u8, u8); 9] = [
    (0o001, 0o013, 1),
    (0o002, 0o007, 0),
    (0o004, 0o026, 1),
    (0o010, 0o051, 0),
    (0o000, 0o132, 1),
    (0o020, 0o224, 0),
    (0o040, 0o150, 1),
    (0o100, 0o340, 0),
    (0o200, 0o320, 1),
];

static HMESS: &[u8] = b"S H O   SSS   TTT   AAA   RRR   SSS\n        S     T     A A   R R   S\nO * T   SSS   T     AAA   RRR   SSS\n          S   T     A A   RR      S\nI N G   SSS   T     A A   R R   SSS\n**********S H O O T I N G   S T A R S******\n           A BRAIN TEASER GAME\nWANT THE RULES? ";
static PAGE1: &[u8] = b"THERE ARE STARS:      *\nAND BLACK HOLES:      .\n\nIN THE UNIVERSE         + + +\n                        + + +\n                        + + +\nYOU SHOOT A STAR\n(NOT A BLACK HOLE)\nBY TYPING ITS NUMBER    1 2 3\n                        4 5 6\n                        7 8 9\nTHAT CHANGES THE STAR TO A BLACK HOLE!\n(TO SEE MORE RULES, TYPE ANY KEY.) ";
static PAGE2: &[u8] = b"EACH STAR IS IN A GALAXY. WHEN YOU\nSHOOT A STAR, EVERYTHING IN ITS GALAXY\nCHANGES. ALL STARS BECOME BLOCK HOLES\nAND ALL BLACK HOLES BECOME STARS.\n\nGALAXIES\n1*.   *2*   .3*   *..  .*.\n**.   ...   .**   4..  *5*\n...   ...   ...   *..  .*.\n\n..*   ...   ...   ...\n..6   **.   ...   .**\n..*   7**   *8*   .*9\n(TYPE ANY KEY FOR LAST PAGE OF RULES.) ";
static PAGE3: &[u8] = b"THE GAME STARTS\nWITH THE UNIVERSE  ...\nLIKE THIS          .*.\n                   ...\n\nYOU WIN WHEN YOU   ***\nCHANGE IT TO THIS  *.*\n                   ***\n\nYOU LOSE IF YOU    ...\nGET THIS           ...\n                   ...\n\nREADY TO PLAY. TYPE ANY KEY TO START\nTHE GAME. GOOD LUCK!";
static MESS1: &[u8] = b"HEY! YOU CAN ONLY SHOOT STARS,\nNOT BLACK HOLES.\nTRY AGAIN!\n";
static MESS2: &[u8] = b"THAT WASNT A VALID STAR NUMBER!\nTRY AGAIN!\n";
static MESS3: &[u8] = b"YOU LOST THE GAME:\nWANT TO SHOOT SOME MORE STARS!  ";
static MESS4: &[u8] = b"YOU WIN! GOOD SHOOTING:\nYOU FIRED  ";
static MESS5: &[u8] = b" SHOTS.\nBEST POSSIBLE SCORE IS 11 SHOTS\nWANT TO SHOOT AGAIN DEADEYE?  ";
static MESS6: &[u8] = b"YOU GIVE UP TOO EASILY!\nWANT TO SHOOT SOME MORE STARS? ";
static MESS7: &[u8] = b"YOUR SHOT? ";

fn puts<H: Hal>(h: &mut H, s: &[u8]) {
    for &c in s {
        h.putc(c);
    }
}

fn crs<H: Hal>(h: &mut H, n: u8) {
    for _ in 0..n {
        h.putc(CR);
    }
}

/// The original's INPUT: read and echo up to CR, return the first key.
fn input<H: Hal>(h: &mut H) -> u8 {
    let first = h.getc();
    h.putc(first);
    loop {
        let c = h.getc();
        h.putc(c);
        if c == CR {
            return first;
        }
    }
}

/// Print the universe: three rows, two CRs between them, `*` or `.` then
/// two spaces per position (the original's DISLOOP).
fn show<H: Hal>(h: &mut H, fringe: u8, centre: u8) {
    let mut bits = fringe;
    for pos in 1..10u8 {
        if pos == 4 || pos == 7 {
            crs(h, 2);
        }
        let star = if pos == 5 {
            centre & 1
        } else {
            let b = bits & 1;
            bits >>= 1;
            b
        };
        h.putc(if star != 0 { b'*' } else { b'.' });
        h.putc(b' ');
        h.putc(b' ');
    }
}

/// One game, from the three CRs to the "again?" answer. True to play again.
fn game<H: Hal>(h: &mut H) -> bool {
    crs(h, 3);
    let mut fringe = 0u8;
    let mut centre = 1u8;
    // Shots fired; the original counts the shot before checking it.
    let mut shots = 0u8;
    loop {
        shots = shots.wrapping_add(1);
        loop {
            show(h, fringe, centre);
            if fringe == 0xFF && centre == 0 {
                puts(h, MESS4);
                print_decimal(h, shots.wrapping_sub(1));
                puts(h, MESS5);
                return input(h) == b'Y';
            }
            if fringe == 0 && centre == 0 {
                puts(h, MESS3);
                return input(h) == b'Y';
            }
            crs(h, 5);
            puts(h, MESS7);
            let c = h.getc();
            h.putc(c);
            crs(h, 3);
            if !(b'1'..=b'9').contains(&c) {
                if c == DEL {
                    // ASSUMPTION (spec SS-1): the intended give-up message.
                    puts(h, MESS6);
                    return input(h) == b'Y';
                }
                puts(h, MESS2);
                continue;
            }
            let (pos, galaxy, flip) = MASKS[(c - b'1') as usize];
            let is_star = if pos == 0 {
                centre == 1
            } else {
                fringe & pos != 0
            };
            if !is_star {
                puts(h, MESS1);
                continue;
            }
            fringe ^= galaxy;
            centre ^= flip;
            break;
        }
    }
}

/// The shot count, without leading zeros (the original's three-digit ASCII
/// counter).
fn print_decimal<H: Hal>(h: &mut H, n: u8) {
    let hundreds = n / 100;
    let tens = n / 10 % 10;
    if hundreds != 0 {
        h.putc(b'0' + hundreds);
    }
    if hundreds != 0 || tens != 0 {
        h.putc(b'0' + tens);
    }
    h.putc(b'0' + n % 10);
}

/// The whole program: heading, optional rules, games until a "no", halt.
pub fn play<H: Hal>(h: &mut H) {
    h.putc(CR);
    puts(h, HMESS);
    if input(h) != b'N' {
        for page in [PAGE1, PAGE2, PAGE3] {
            puts(h, page);
            h.getc();
        }
    }
    while game(h) {}
    h.halt();
}

// Keep LF referenced: the texts use it as the line separator.
const _: u8 = LF;
