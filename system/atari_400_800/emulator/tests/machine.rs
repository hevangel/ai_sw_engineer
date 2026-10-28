//! Directed whole-machine tests for the Atari 400/800 emulator.
//!
//! Programs are hand-assembled byte arrays; each byte sequence is commented
//! with the instruction it encodes. These tests verify bus decode and
//! hardware mirrors, machine timing (WSYNC, VCOUNT, vertical-blank NMI),
//! the POKEY IRQ/RANDOM models, the PIA register semantics, machine
//! profiles and run-to-run determinism.

use atari_400_800_emulator::{Machine, MachineProfile};

/// Machine with a program loaded in RAM at `pc`, reset to that address.
fn machine_with(pc: u16, program: &[u8]) -> Machine {
    let mut m = Machine::new(MachineProfile::Atari800);
    m.load(pc, program);
    m.reset();
    m.force_pc(pc);
    m
}

/// 8 KiB OS ROM image with an NMI handler at $E000 (INC $0200; RTI) and the
/// NMI vector at $FFFA pointing at it.
fn os_rom_with_nmi_handler() -> [u8; 0x2000] {
    let mut os = [0xFF; 0x2000];
    os[0..4].copy_from_slice(&[0xEE, 0x00, 0x02, 0x40]); // $E000: INC $0200 ; RTI
    os[0x1FFA] = 0x00; // $FFFA: NMI vector low
    os[0x1FFB] = 0xE0; // $FFFB: NMI vector high -> $E000
    os
}

#[test]
fn reset_fetches_os_rom_reset_vector() {
    let mut os = [0xFF; 0x2000];
    os[0] = 0xA9; // $E000: LDA #$2B
    os[1] = 0x2B;
    os[2] = 0x8D; // $E002: STA $0600
    os[3] = 0x00;
    os[4] = 0x06;
    os[5] = 0x4C; // $E005: JMP $E000
    os[6] = 0x00;
    os[7] = 0xE0;
    os[0x1FFC] = 0x00; // $FFFC: reset vector -> $E000
    os[0x1FFD] = 0xE0;
    let mut m = Machine::new(MachineProfile::Atari800);
    m.load_os_rom(&os).unwrap();
    m.reset();
    m.run_frames(1);
    // The reset vector fetched $E000; the program's loop spans $E000-$E005,
    // so the exact PC at frame end depends on the line boundary.
    assert!(
        matches!(m.pc(), 0xE000 | 0xE005),
        "reset must enter the OS ROM program, pc=${:04X}",
        m.pc()
    );
    assert_eq!(m.peek(0x0600), 0x2B);
}

#[test]
fn wsync_halts_cpu_until_end_of_scanline() {
    // $2000: STA $D40A (WSYNC) ; JMP $2000
    let prog = [0x8D, 0x0A, 0xD4, 0x4C, 0x00, 0x20];
    let mut m = machine_with(0x2000, &prog);
    m.run_frames(1);
    // A full frame is 262 * 114 = 29868 virtual cycles regardless of halts.
    assert_eq!(m.virtual_cycles(), 29868);
    // Each JMP (3 cycles) is followed by a STA WSYNC (4 cycles) that halts
    // the CPU for the rest of the line, so the CPU executed far fewer than
    // a full frame of busy cycles.
    assert!(
        m.cpu_cycles() < 5000,
        "WSYNC must halt the CPU; cpu_cycles={}",
        m.cpu_cycles()
    );
}

#[test]
fn vcount_reads_scanline_divided_by_two() {
    let mut m = Machine::new(MachineProfile::Atari800);
    m.seek_to_scanline(240);
    assert_eq!(m.peek(0xD40B), 120); // VCOUNT = scanline >> 1
    m.seek_to_scanline(242);
    assert_eq!(m.peek(0xD40B), 121);
}

#[test]
fn vbi_status_sets_at_line_248() {
    let mut m = Machine::new(MachineProfile::Atari800);
    m.seek_to_scanline(247);
    assert_eq!(m.peek(0xD40F) & 0x40, 0, "no VBI status before line 248");
    m.seek_to_scanline(249);
    assert_eq!(m.peek(0xD40F) & 0x40, 0x40, "NMIST bit 6 set at line 248");
    assert_eq!(m.peek(0xD40B), 124); // VCOUNT = 249 >> 1
}

#[test]
fn vbi_nmi_fires_once_per_nmires_and_status_tracks_it() {
    let os = os_rom_with_nmi_handler();
    // $2000: LDA #$40 ; STA $D40E (NMIEN = VBI) ; JMP $2005
    let prog = [0xA9, 0x40, 0x8D, 0x0E, 0xD4, 0x4C, 0x05, 0x20];
    let mut m = Machine::new(MachineProfile::Atari800);
    m.load_os_rom(&os).unwrap();
    m.load(0x2000, &prog);
    m.reset();
    m.force_pc(0x2000);
    m.run_frames(3);
    // The NMI line stays asserted until software writes NMIRES, so the
    // handler runs exactly once across three frames.
    assert_eq!(m.peek(0x0200), 1, "one NMI until NMIRES re-arms the edge");
    assert_ne!(m.peek(0xD40F) & 0x40, 0, "NMIST still shows the VBI");
    m.poke(0xD40F, 0x00); // NMIRES
    assert_eq!(m.peek(0xD40F), 0x00);
    m.run_frames(1);
    assert_eq!(m.peek(0x0200), 2, "next VBI is serviced after NMIRES");
}

#[test]
fn pokey_irqenable_marks_pending_and_vectors() {
    let mut os = [0xFF; 0x2000];
    // $E010 IRQ handler: INC $10 ; LDA #$00 ; STA $D20E ; RTI
    os[0x10..0x18].copy_from_slice(&[0xE6, 0x10, 0xA9, 0x00, 0x8D, 0x0E, 0xD2, 0x40]);
    os[0x1FFE] = 0x10; // $FFFE: IRQ vector -> $E010
    os[0x1FFF] = 0xE0;
    // $2000: SEI ; LDA #$00 ; STA $11 ; LDA #$40 ; STA $D20E ; CLI ; NOP ;
    //        JMP $200C
    let prog = [
        0x78, // $2000: SEI
        0xA9, 0x00, // $2001: LDA #$00
        0x85, 0x11, // $2003: STA $11
        0xA9, 0x40, // $2005: LDA #$40
        0x8D, 0x0E, 0xD2, // $2007: STA $D20E (IRQEN, marks pending)
        0x58, // $200A: CLI
        0xEA, // $200B: NOP (IRQ taken at the end of this instruction)
        0x4C, 0x0C, 0x20, // $200C: JMP $200C
    ];
    let mut m = Machine::new(MachineProfile::Atari800);
    m.load_os_rom(&os).unwrap();
    m.load(0x2000, &prog);
    m.reset();
    m.force_pc(0x2000);
    m.run_cycles(2000);
    assert_eq!(m.peek(0x10), 1, "handler must run exactly once");
    assert_eq!(m.pc(), 0x200C, "RTI returns after the NOP");
    let pokey = &m.bus().pokey;
    assert_eq!(pokey.irq_en, 0x00, "handler disabled IRQEN");
    assert_eq!(
        pokey.irq_st & 0x40,
        0x00,
        "keyboard status still pending-low"
    );
}

#[test]
fn pokey_random_is_deterministic_across_machines() {
    // $2000: LDX #$00 ; loop: LDA $D20A ; STA $0600,X ; INX ; CPX #$08 ;
    //        BNE loop ; JMP $200B
    let prog = [
        0xA2, 0x00, // $2000: LDX #$00
        0xAD, 0x0A, 0xD2, // $2002: LDA $D20A (RANDOM)
        0x9D, 0x00, 0x06, // $2005: STA $0600,X
        0xE8, // $2008: INX
        0xE0, 0x08, // $2009: CPX #$08
        0xD0, 0xF7, // $200B... (see below) BNE $2002
        0x4C, 0x0B, 0x20, // $200B: JMP $200B
    ];
    let mut a = machine_with(0x2000, &prog);
    let mut b = machine_with(0x2000, &prog);
    a.run_frames(1);
    b.run_frames(1);
    let mut values = [0u8; 8];
    for (i, v) in values.iter_mut().enumerate() {
        *v = a.peek(0x0600 + i as u16);
        assert_eq!(*v, b.peek(0x0600 + i as u16), "run must be deterministic");
    }
    assert!(
        values.windows(2).any(|w| w[0] != w[1]),
        "RANDOM must step between reads: {values:02X?}"
    );
}

#[test]
fn gtia_pia_and_hardware_mirror_decode() {
    let mut m = Machine::new(MachineProfile::Atari800);
    assert_eq!(m.peek(0xD010), 1, "TRIG0 released");
    assert_eq!(m.peek(0xD01F), 0x07, "CONSOL keys up");
    assert_eq!(m.peek(0xD014), 0x0F, "PAL flag register: NTSC");
    // PIA 6520 semantics: PACTL=0 selects DDR at $D300, PACTL=4 the port.
    m.poke(0xD302, 0x00);
    m.poke(0xD300, 0x0F); // DDRA = $0F
    m.poke(0xD302, 0x04);
    assert_eq!(m.peek(0xD300), 0xF0, "out=$00, in=$FF, DDR=$0F");
    // Registers repeat through their slot: ANTIC DLISTL at $D412, POKEY
    // IRQEN at $D21E, GTIA COLBK at $D03A.
    m.poke(0xD412, 0x34);
    assert_eq!(m.bus().antic.dlist & 0xFF, 0x34);
    m.poke(0xD21E, 0x40);
    assert_eq!(m.bus().pokey.irq_en, 0x40);
    m.poke(0xD03A, 0x99);
    assert_eq!(m.bus().gtia.regs[0x1A], 0x99);
}

#[test]
fn profiles_cart_and_open_bus() {
    let mut m400 = Machine::new(MachineProfile::Atari400);
    m400.poke(0x3FFF, 0x77);
    assert_eq!(m400.peek(0x3FFF), 0x77, "16 KiB RAM top");
    m400.poke(0x4000, 0x88);
    assert_eq!(m400.peek(0x4000), 0xFF, "above RAM top: open bus");
    let cart = vec![0xAB_u8; 0x2000];
    m400.load_cart(&cart).unwrap();
    assert_eq!(m400.peek(0xA000), 0xAB, "8K cartridge overlays $A000");
    m400.poke(0xA000, 0x55);
    assert_eq!(m400.peek(0xA000), 0xAB, "writes to cartridge are lost");

    let mut m800 = Machine::new(MachineProfile::Atari800);
    m800.poke(0xA000, 0x12);
    assert_eq!(
        m800.peek(0xA000),
        0x12,
        "48K machine: cart slot area is RAM"
    );
    assert_eq!(m800.peek(0xC000), 0xFF, "$C000-$CFFF: open bus");
    assert_eq!(m800.peek(0xD600), 0xFF, "$D500-$D7FF: open bus");
}

#[test]
fn identical_runs_are_bit_identical() {
    let build = || {
        let os = os_rom_with_nmi_handler();
        let prog = [0xA9, 0x40, 0x8D, 0x0E, 0xD4, 0x4C, 0x05, 0x20];
        let mut m = Machine::new(MachineProfile::Atari800);
        m.load_os_rom(&os).unwrap();
        m.load(0x2000, &prog);
        m.reset();
        m.force_pc(0x2000);
        m
    };
    let mut a = build();
    let mut b = build();
    a.run_frames(2);
    b.run_frames(2);
    assert_eq!(a.cpu_cycles(), b.cpu_cycles());
    assert_eq!(a.virtual_cycles(), b.virtual_cycles());
    assert_eq!(a.pc(), b.pc());
    assert_eq!(a.peek(0x0200), b.peek(0x0200));
    assert_eq!(a.peek(0xD40F), b.peek(0xD40F));
    assert_eq!(a.peek(0xD20A), b.peek(0xD20A));
}
