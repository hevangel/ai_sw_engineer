//! Hand-assembled programs for what the single-instruction silicon vectors
//! cannot show: control flow, the stack, interrupts and their shadows, `REP`
//! interruption, and the machine's memory map. Every expected value is worked
//! out from Intel's *8086 Family User's Manual*, not copied from the core.

use system_emulator::cpu::{Bus, Cpu, Step, AF, CF, IF, PF, ZF};
use system_emulator::machine::Machine;

const CODE: u32 = 0x0100;

/// A 64 KiB machine with `code` at 0000:0100, SS:SP = 0000:8000, halted by HLT.
fn machine(code: &[u8]) -> Machine {
    let mut m = Machine::new(64);
    m.load_ram(CODE, code).unwrap();
    m.cpu.s = [0; 4];
    m.cpu.ip = CODE as u16;
    m.cpu.r[4] = 0x8000;
    m
}

fn run_to_halt(m: &mut Machine) {
    assert_eq!(m.run(100_000), Step::Halted, "program did not halt");
}

fn word(m: &mut Machine, addr: u32) -> u16 {
    u16::from(m.read(addr)) | (u16::from(m.read(addr + 1)) << 8)
}

#[test]
fn loop_sums_one_to_ten() {
    // mov cx,10; xor ax,ax; L: add ax,cx; loop L; hlt
    let mut m = machine(&[0xB9, 0x0A, 0x00, 0x31, 0xC0, 0x01, 0xC8, 0xE2, 0xFC, 0xF4]);
    run_to_halt(&mut m);
    assert_eq!((m.cpu.r[0], m.cpu.r[1]), (55, 0));
}

#[test]
fn add_sets_documented_flags() {
    // mov al,FFh; add al,1 -> AL=0 with CF, PF, AF and ZF set; SF and OF clear.
    let mut m = machine(&[0xB0, 0xFF, 0x04, 0x01, 0xF4]);
    run_to_halt(&mut m);
    assert_eq!(m.cpu.r[0] & 0xFF, 0);
    assert_eq!(m.cpu.flags() & 0x8D5, CF | PF | AF | ZF);
}

#[test]
fn call_and_ret_use_the_stack() {
    // call +3; hlt; nop; nop; mov ax,1234h; ret
    let mut m = machine(&[0xE8, 0x03, 0x00, 0xF4, 0x90, 0x90, 0xB8, 0x34, 0x12, 0xC3]);
    run_to_halt(&mut m);
    assert_eq!(m.cpu.r[0], 0x1234);
    assert_eq!(m.cpu.r[4], 0x8000, "SP restored by RET");
    assert_eq!(
        word(&mut m, 0x7FFE),
        0x0103,
        "return address left on the stack"
    );
}

#[test]
fn rep_movsb_copies_forward_and_backward() {
    let mut m = machine(&[0xF3, 0xA4, 0xF4]); // rep movsb; hlt
    m.load_ram(0x200, b"HELLO").unwrap();
    m.cpu.r = [0, 5, 0, 0, 0x8000, 0, 0x200, 0x300];
    run_to_halt(&mut m);
    let copied: Vec<u8> = (0..5).map(|i| m.read(0x300 + i)).collect();
    assert_eq!(copied, b"HELLO");
    assert_eq!((m.cpu.r[1], m.cpu.r[6], m.cpu.r[7]), (0, 0x205, 0x305));

    let mut m = machine(&[0xFD, 0xF3, 0xA4, 0xF4]); // std; rep movsb; hlt
    m.load_ram(0x200, b"HELLO").unwrap();
    m.cpu.r = [0, 5, 0, 0, 0x8000, 0, 0x204, 0x304];
    run_to_halt(&mut m);
    let copied: Vec<u8> = (0..5).map(|i| m.read(0x300 + i)).collect();
    assert_eq!(copied, b"HELLO");
    assert_eq!((m.cpu.r[6], m.cpu.r[7]), (0x1FF, 0x2FF));
}

#[test]
fn int_pushes_flags_cs_ip_and_iret_restores() {
    let mut m = machine(&[0xFB, 0xCD, 0x21, 0xF4]); // sti; int 21h; hlt
    m.load_ram(0x84, &[0x00, 0x03, 0x00, 0x00]).unwrap(); // vector 21h -> 0000:0300
    m.load_ram(0x300, &[0x89, 0xE3, 0x40, 0xCF]).unwrap(); // mov bx,sp; inc ax; iret
    run_to_halt(&mut m);
    assert_eq!(
        m.cpu.r[3], 0x7FFA,
        "SP inside the handler: three words pushed"
    );
    assert_eq!(m.cpu.r[0], 1);
    assert_eq!(m.cpu.r[4], 0x8000);
    assert_eq!(
        word(&mut m, 0x7FFA),
        0x0103,
        "return IP is the byte after INT"
    );
    assert_eq!(word(&mut m, 0x7FFC), 0x0000, "CS");
    assert_eq!(
        word(&mut m, 0x7FFE),
        0xF202,
        "FLAGS with IF set, pushed last-in-first"
    );
    assert_ne!(m.cpu.flags() & IF, 0, "IRET restores IF");
}

#[test]
fn divide_error_returns_to_the_next_instruction() {
    // mov ax,100h; mov bl,0; div bl; hlt. Vector 0 -> a lone HLT.
    let mut m = machine(&[0xB8, 0x00, 0x01, 0xB3, 0x00, 0xF6, 0xF3, 0xF4]);
    m.load_ram(0, &[0x00, 0x03, 0x00, 0x00]).unwrap();
    m.load_ram(0x300, &[0xF4]).unwrap();
    run_to_halt(&mut m);
    assert_eq!(m.cpu.ip, 0x301);
    assert_eq!(
        word(&mut m, 0x7FFA),
        0x0107,
        "8086 saves the address after DIV"
    );
}

#[test]
fn sti_defers_interrupts_by_one_instruction() {
    // sti; nop; nop; hlt with INTR already asserted. Handler: inc ax; iret.
    let mut m = machine(&[0xFB, 0x90, 0x90, 0xF4]);
    m.load_ram(0x80, &[0x00, 0x03, 0x00, 0x00]).unwrap(); // vector 20h
    m.load_ram(0x300, &[0x40, 0xCF]).unwrap();
    m.raise_intr(0x20);
    assert_eq!(m.step(), Step::Ran); // STI
    assert_eq!(m.step(), Step::Ran); // first NOP runs: the interrupt shadow
    assert_eq!((m.cpu.r[0], m.cpu.ip), (0, 0x102));
    assert_eq!(m.step(), Step::Ran); // interrupt entry
    assert_eq!(m.cpu.ip, 0x300);
    assert_eq!(word(&mut m, 0x7FFA), 0x0102, "return to the second NOP");
    run_to_halt(&mut m);
    assert_eq!(m.cpu.r[0], 1);
}

#[test]
fn nmi_ignores_if_and_hlt_wakes_on_interrupts() {
    let mut m = machine(&[0xF4, 0xF4]);
    m.load_ram(0x08, &[0x00, 0x03, 0x00, 0x00]).unwrap(); // NMI vector 2
    m.load_ram(0x300, &[0xCF]).unwrap();
    assert_eq!(m.step(), Step::Ran); // HLT
    assert_eq!(m.step(), Step::Halted);
    m.raise_nmi(); // IF is clear
    assert_eq!(m.step(), Step::Ran);
    assert_eq!(m.cpu.ip, 0x300);
    assert_eq!(
        word(&mut m, 0x7FFA),
        0x0101,
        "returns to the instruction after HLT"
    );
}

#[test]
fn trap_flag_single_steps() {
    // inc ax x4 with TF set; handler for INT 1: inc word [600h]; iret.
    let mut m = machine(&[0x40, 0x40, 0x40, 0x40]);
    m.load_ram(0x04, &[0x00, 0x05, 0x00, 0x00]).unwrap();
    m.load_ram(0x500, &[0xFF, 0x06, 0x00, 0x06, 0xCF]).unwrap();
    m.cpu.set_flags(m.cpu.flags() | 0x100);
    for _ in 0..200 {
        m.step();
        if m.cpu.r[0] == 3 {
            break;
        }
    }
    assert_eq!(m.cpu.r[0], 3);
    assert_eq!(
        word(&mut m, 0x600),
        2,
        "one trap after each of the first two instructions"
    );
}

/// A flat 1 MiB memory that asserts INTR after a number of reads.
struct Probe {
    mem: Vec<u8>,
    reads: u64,
    intr_after: u64,
    taken: bool,
}

impl Bus for Probe {
    fn read(&mut self, a: u32) -> u8 {
        self.reads += 1;
        self.mem[a as usize]
    }
    fn write(&mut self, a: u32, v: u8) {
        self.mem[a as usize] = v;
    }
    fn port_in(&mut self, _: u16) -> u8 {
        0xFF
    }
    fn port_out(&mut self, _: u16, _: u8) {}
    fn intr_pending(&self) -> bool {
        !self.taken && self.reads >= self.intr_after
    }
    fn inta(&mut self) -> u8 {
        self.taken = true;
        0x20
    }
}

#[test]
fn interrupted_rep_resumes_at_the_prefix_and_finishes() {
    let mut bus = Probe {
        mem: vec![0; 1 << 20],
        reads: 0,
        intr_after: 6,
        taken: false,
    };
    bus.mem[0x100..0x103].copy_from_slice(&[0xF3, 0xA4, 0xF4]); // rep movsb; hlt
    bus.mem[0x200..0x20A].copy_from_slice(b"0123456789");
    bus.mem[0x80..0x84].copy_from_slice(&[0x00, 0x04, 0x00, 0x00]); // vector 20h -> 0400
    bus.mem[0x400] = 0xCF; // iret
    let mut cpu = Cpu::new();
    cpu.s = [0; 4];
    cpu.ip = 0x100;
    cpu.r = [0, 10, 0, 0, 0x8000, 0, 0x200, 0x300];
    cpu.set_flags(IF | 0xF002);

    assert_eq!(cpu.step(&mut bus), Step::Ran);
    assert_eq!(cpu.ip, 0x100, "interrupt will return to the REP prefix");
    let left = cpu.r[1];
    assert!(left > 0 && left < 10, "stopped part-way, CX={left}");
    assert_eq!(cpu.step(&mut bus), Step::Ran); // interrupt entry
    assert_eq!(cpu.ip, 0x400);
    let mut steps = 0;
    while cpu.step(&mut bus) == Step::Ran {
        steps += 1;
        assert!(steps < 100);
    }
    assert_eq!(&bus.mem[0x300..0x30A], b"0123456789");
    assert_eq!(cpu.r[1], 0);
}

#[test]
fn memory_map_matches_the_5150() {
    let mut m = Machine::new(64);
    // Reset vector: FFFF:0000 holds a far jump to 0000:0100.
    let mut rom = vec![0xFF; 8192];
    rom[8192 - 16..8192 - 11].copy_from_slice(&[0xEA, 0x00, 0x01, 0x00, 0x00]);
    m.load_rom_top(&rom).unwrap();
    m.load_ram(CODE, &[0xF4]).unwrap();
    assert_eq!(m.step(), Step::Ran);
    assert_eq!((m.cpu.s[1], m.cpu.ip), (0, 0x100));
    // ROM ignores writes; unmapped memory reads FFh; CGA memory is RAM.
    let mut m = machine(&[
        0xB8, 0x00, 0xF6, 0x8E, 0xD8, // ds = F600h
        0xC6, 0x06, 0x10, 0x00, 0x55, // mov byte [10h],55h  (ROM)
        0xA0, 0x10, 0x00, //             mov al,[10h]
        0xB8, 0x00, 0xB8, 0x8E, 0xC0, // es = B800h
        0x26, 0xC6, 0x06, 0x00, 0x00, 0x7E, // mov byte es:[0],7Eh
        0x26, 0x8A, 0x1E, 0x00, 0x00, // mov bl,es:[0]
        0xB8, 0x00, 0x20, 0x8E, 0xD8, // ds = 2000h (beyond 64 KiB of RAM)
        0xA0, 0x00, 0x00, //             mov al,[0]
        0xF4,
    ]);
    m.load_rom_top(&[0x12; 16]).unwrap();
    run_to_halt(&mut m);
    assert_eq!(m.cpu.r[3] & 0xFF, 0x7E, "CGA memory reads back");
    assert_eq!(m.cpu.r[0] & 0xFF, 0xFF, "unmapped memory reads FFh");
}

#[test]
fn segments_wrap_within_64k_and_at_one_megabyte() {
    // mov ax,[FFFFh] with DS=1000h: low byte at 1FFFFh, high byte at 10000h.
    let mut m2 = Machine::new(128);
    m2.load_ram(
        CODE,
        &[0xB8, 0x00, 0x10, 0x8E, 0xD8, 0xA1, 0xFF, 0xFF, 0xF4],
    )
    .unwrap();
    m2.load_ram(0x1FFFF, &[0x34]).unwrap();
    m2.load_ram(0x10000, &[0x12]).unwrap();
    m2.cpu.s = [0; 4];
    m2.cpu.ip = CODE as u16;
    run_to_halt(&mut m2);
    assert_eq!(m2.cpu.r[0], 0x1234);
    // mov al,[10h] with DS=FFFFh: FFFF0h + 10h wraps to physical 00000h.
    let mut m = machine(&[0xB8, 0xFF, 0xFF, 0x8E, 0xD8, 0xA0, 0x10, 0x00, 0xF4]);
    m.load_ram(0, &[0x5A]).unwrap();
    run_to_halt(&mut m);
    assert_eq!(m.cpu.r[0] & 0xFF, 0x5A);
}

#[test]
fn in_and_out_reach_the_io_bus() {
    // mov al,55h; out 80h,al; in al,81h; hlt (nothing on the bus: FFh)
    let mut m = machine(&[0xB0, 0x55, 0xE6, 0x80, 0xE4, 0x81, 0xF4]);
    run_to_halt(&mut m);
    assert_eq!(m.board.io_log, vec![(0x80, 0x55)]);
    assert_eq!(m.cpu.r[0] & 0xFF, 0xFF);
}
