//! The tiny-model `.EXE` writer: an MZ header around the `.COM` image.

use i8086_ld::exe;

fn word(h: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([h[at], h[at + 1]])
}

#[test]
fn the_header_describes_a_tiny_model_program() {
    let image: Vec<u8> = (0..1000u32).map(|i| i as u8).collect();
    let out = exe(&image, 0x100, 0x110, 300).unwrap();
    assert_eq!(&out[..2], b"MZ");
    assert_eq!(out.len(), 512 + image.len());
    assert_eq!(
        &out[512..],
        &image[..],
        "the image follows the header unchanged"
    );
    assert_eq!(
        word(&out, 2),
        (out.len() % 512) as u16,
        "bytes in the last page"
    );
    assert_eq!(word(&out, 4), out.len().div_ceil(512) as u16, "pages");
    assert_eq!(word(&out, 6), 0, "no relocations");
    assert_eq!(word(&out, 8), 32, "a header of one sector: 32 paragraphs");
    // what the program needs after the file's bytes: 300 more
    let min = word(&out, 10);
    assert_eq!(u32::from(min), 300u32.div_ceil(16));
    assert_eq!(word(&out, 12), 0xFFFF, "maximum: all memory");
    // SP is inside what the program asked for: 0100h + 1000 + 300, even
    assert_eq!(word(&out, 16), (0x100 + 1000 + 300) & !1);
    assert_eq!(
        (word(&out, 14), word(&out, 22)),
        (0xFFF0, 0xFFF0),
        "SS and CS: load segment - 10h"
    );
    assert_eq!(word(&out, 20), 0x110, "IP");
    assert_eq!(word(&out, 24), 0x1C, "relocation table offset");
}

#[test]
fn images_that_are_not_a_tiny_model_program_are_refused() {
    assert!(exe(&[0x90; 16], 0x200, 0x200, 0)
        .unwrap_err()
        .contains("starts at 0x0100"));
    assert!(exe(&vec![0; 0x10000], 0x100, 0x100, 0)
        .unwrap_err()
        .contains("do not fit"));
    assert!(exe(&[0x90; 16], 0x100, 0x400, 0)
        .unwrap_err()
        .contains("outside the image"));
    assert!(exe(&[0x90; 16], 0x100, 0xFF, 0)
        .unwrap_err()
        .contains("outside the image"));
}
