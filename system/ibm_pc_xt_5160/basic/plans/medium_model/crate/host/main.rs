#[path = "../src/logic.rs"]
mod logic;
#[path = "../src/pad.rs"]
mod pad;
struct H(Vec<u8>, Vec<u8>);
impl logic::Hal for H {
    fn putc(&mut self, c: u8) {
        self.0.push(c)
    }
    fn far_r8(&mut self, _s: u16, o: u16) -> u8 {
        self.1[o as usize]
    }
    fn far_w8(&mut self, _s: u16, o: u16, v: u8) {
        self.1[o as usize] = v
    }
}
fn main() {
    let pad = std::env::args().nth(1).is_some();
    let mut h = H(Vec::new(), vec![0; 65536]);
    logic::run(
        &mut h,
        0x0BA5_1C00,
        0,
        if pad {
            Some(pad::all as fn(u32) -> u32)
        } else {
            None
        },
    );
    println!("{}", String::from_utf8(h.0).unwrap());
}
