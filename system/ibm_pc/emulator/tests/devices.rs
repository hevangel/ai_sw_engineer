//! Peripheral models against their datasheets (Intel 8259A, 8253, 8237A,
//! 8272/uPD765) and the IBM keyboard handshake. Expected values are worked out
//! from the datasheets, not taken from the models.

use system_emulator::dma::{Dma, Kind};
use system_emulator::fdc::{Fdc, DISK_BYTES, SECTOR_BYTES};
use system_emulator::pic::Pic;
use system_emulator::pit::Pit;
use system_emulator::ppi::Keyboard;

/// Output of timer channel 0 for `n` ticks, sampled after each tick.
fn pit_wave(pit: &mut Pit, n: usize) -> Vec<bool> {
    (0..n)
        .map(|_| {
            pit.advance(1);
            pit.out(0)
        })
        .collect()
}

#[test]
fn pit_mode2_pulses_low_once_per_period() {
    let mut pit = Pit::new();
    pit.write(3, 0x14); // channel 0, LSB only, mode 2, binary
    pit.write(0, 5);
    pit.advance(1); // the count is transferred on the first clock
    let wave = pit_wave(&mut pit, 20);
    let lows: Vec<usize> = wave
        .iter()
        .enumerate()
        .filter(|(_, o)| !**o)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(lows.len(), 4, "one low tick per 5-tick period: {wave:?}");
    assert!(lows.windows(2).all(|w| w[1] - w[0] == 5));
}

#[test]
fn pit_mode3_is_a_square_wave_and_zero_means_65536() {
    let mut pit = Pit::new();
    pit.write(3, 0x16); // channel 0, LSB, mode 3
    pit.write(0, 4);
    // The count transfers on the first clock; high for 2 clocks, then low for 2.
    let wave = pit_wave(&mut pit, 8);
    assert_eq!(wave, [true, true, false, false, true, true, false, false]);

    let mut pit = Pit::new();
    pit.write(3, 0x36); // LSB then MSB, mode 3
    pit.write(0, 0);
    pit.write(0, 0);
    pit.advance(1 + 32767);
    assert!(pit.out(0), "high for the first 32768 ticks");
    pit.advance(2);
    assert!(!pit.out(0), "low after half of 65536");
}

#[test]
fn pit_mode0_goes_high_at_terminal_count_and_latches_reads() {
    let mut pit = Pit::new();
    pit.write(3, 0x30); // channel 0, LSB/MSB, mode 0
    pit.write(0, 10);
    pit.write(0, 0);
    assert!(!pit.out(0));
    pit.advance(4);
    pit.write(3, 0x00); // latch channel 0
    let lo = pit.read(0);
    let hi = pit.read(0);
    assert_eq!(u16::from(lo) | (u16::from(hi) << 8), 10 - 3);
    pit.advance(20);
    assert!(pit.out(0));
}

#[test]
fn pic_priority_masks_and_end_of_interrupt() {
    let mut pic = Pic::new();
    for (a0, v) in [(false, 0x13), (true, 0x08), (true, 0x09), (true, 0x00)] {
        pic.write(a0, v);
    }
    assert!(!pic.int());
    pic.set_line(1, true);
    pic.set_line(0, true);
    assert!(pic.int());
    assert_eq!(pic.ack(), 0x08, "IR0 outranks IR1");
    assert!(!pic.int(), "IR1 waits while IR0 is in service");
    pic.write(false, 0x20); // non-specific EOI
    assert!(pic.int());
    assert_eq!(pic.ack(), 0x09);
    pic.write(false, 0x20);
    // A line that stays high is not a new edge.
    pic.set_line(0, true);
    assert!(!pic.int());
    pic.set_line(0, false);
    pic.set_line(0, true);
    assert!(pic.int());
    pic.write(true, 0x01); // mask IR0
    assert!(!pic.int());
    assert_eq!(pic.read(true), 0x01);
}

#[test]
fn dma_registers_use_the_byte_flip_flop() {
    let mut dma = Dma::new();
    dma.write(0xC, 0); // clear flip-flop
    dma.write(4, 0x34); // channel 2 address, low then high
    dma.write(4, 0x12);
    dma.write(5, 0xFF); // channel 2 count
    dma.write(5, 0x01);
    dma.write(0xC, 0);
    assert_eq!((dma.read(4), dma.read(4)), (0x34, 0x12));
    dma.write(0xB, 0x46); // single, write to memory, channel 2
    assert_eq!(dma.kind(2), Kind::Write);
    dma.write_page(1, 0x03); // port 81h is channel 2's page
    assert_eq!(dma.address(2), 0x3_1234);
    assert!(!dma.channel_ready(2), "masked after reset");
    dma.write(0xA, 0x02); // unmask channel 2
    assert!(dma.channel_ready(2));
    // 0x1FF + 1 bytes, then terminal count masks the channel again.
    for _ in 0..0x1FF {
        assert!(!dma.advance(2));
    }
    assert!(dma.advance(2));
    assert!(!dma.channel_ready(2));
    assert_eq!(dma.read(8) & 4, 4, "TC status bit for channel 2");
}

fn disk() -> Vec<u8> {
    (0..DISK_BYTES).map(|i| (i % 251) as u8).collect()
}

fn command(fdc: &mut Fdc, bytes: &[u8]) {
    for &b in bytes {
        assert_eq!(
            fdc.status() & 0xC0,
            0x80,
            "controller ready for a command byte"
        );
        fdc.write_data(b);
    }
}

fn results(fdc: &mut Fdc, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| {
            assert_eq!(fdc.status() & 0xC0, 0xC0, "result byte ready");
            fdc.read_data()
        })
        .collect()
}

#[test]
fn fdc_reset_raises_one_polling_interrupt_and_seek_completes() {
    let mut fdc = Fdc::new();
    fdc.mount(0, disk(), false).unwrap();
    fdc.write_dor(0x08); // reset asserted, DMA/IRQ enabled
    fdc.write_dor(0x0C); // released
    assert!(!fdc.irq());
    fdc.advance(10_000);
    assert!(fdc.irq());
    command(&mut fdc, &[0x08]);
    assert_eq!(results(&mut fdc, 2), [0xC0, 0x00], "ready-change, unit 0");
    assert!(!fdc.irq(), "a single polling interrupt");
    command(&mut fdc, &[0x08]);
    assert_eq!(
        results(&mut fdc, 1),
        [0x80],
        "no interrupt pending: invalid"
    );

    fdc.write_dor(0x1C); // motor on, drive 0
    command(&mut fdc, &[0x03, 0xDF, 0x02]); // specify
    command(&mut fdc, &[0x0F, 0x00, 0x05]); // seek to cylinder 5
    assert_eq!(fdc.status() & 0x01, 1, "drive 0 busy seeking");
    fdc.advance(1_000_000);
    assert!(fdc.irq());
    command(&mut fdc, &[0x08]);
    assert_eq!(
        results(&mut fdc, 2),
        [0x20, 0x05],
        "seek end, present cylinder 5"
    );
    command(&mut fdc, &[0x04, 0x00]);
    assert_eq!(
        results(&mut fdc, 1)[0] & 0xF0,
        0x20,
        "ST3: ready, not on track 0, not write protected"
    );
    command(&mut fdc, &[0x07, 0x00]); // recalibrate
    fdc.advance(5_000_000);
    command(&mut fdc, &[0x08]);
    assert_eq!(results(&mut fdc, 2), [0x20, 0x00]);
}

#[test]
fn fdc_reads_a_sector_through_dma_with_terminal_count() {
    let image = disk();
    let mut fdc = Fdc::new();
    fdc.mount(0, image.clone(), false).unwrap();
    fdc.write_dor(0x08);
    fdc.write_dor(0x1C);
    fdc.advance(10_000);
    command(&mut fdc, &[0x08]); // the BIOS services the reset polling interrupt first
    results(&mut fdc, 2);
    // MFM read data: C=0 H=0 R=3 N=2, EOT=8, GPL=2A, DTL=FF.
    command(
        &mut fdc,
        &[0x46, 0x00, 0x00, 0x00, 0x03, 0x02, 0x08, 0x2A, 0xFF],
    );
    assert_eq!(fdc.status() & 0xD0, 0x10, "busy in the execution phase");
    let mut data = Vec::new();
    for _ in 0..3_000_000 {
        fdc.advance(100);
        while fdc.drq() {
            if data.len() == SECTOR_BYTES - 1 {
                fdc.terminal_count(); // EOP with the last byte
            }
            data.push(fdc.dma_read());
        }
        if data.len() == SECTOR_BYTES {
            break;
        }
    }
    assert_eq!(data, &image[2 * SECTOR_BYTES..3 * SECTOR_BYTES]);
    assert!(fdc.irq());
    assert_eq!(
        results(&mut fdc, 7),
        [0x00, 0x00, 0x00, 0, 0, 4, 2],
        "next sector after TC"
    );
    assert!(!fdc.irq());
}

#[test]
fn fdc_reports_errors_without_media_and_on_bad_sector() {
    let mut fdc = Fdc::new();
    fdc.write_dor(0x08);
    fdc.write_dor(0x1C);
    command(
        &mut fdc,
        &[0x46, 0x00, 0x00, 0x00, 0x01, 0x02, 0x08, 0x2A, 0xFF],
    );
    fdc.advance(5_000_000);
    let r = results(&mut fdc, 7);
    assert_eq!(r[0] & 0xC0, 0x40, "abnormal termination without a disk");
    assert_ne!(r[1], 0);

    fdc.mount(0, disk(), true).unwrap();
    command(
        &mut fdc,
        &[0x45, 0x00, 0x00, 0x00, 0x01, 0x02, 0x08, 0x2A, 0xFF],
    ); // write
    fdc.advance(5_000_000);
    let r = results(&mut fdc, 7);
    assert_eq!((r[0] & 0xC0, r[1] & 0x02), (0x40, 0x02), "write protected");
    command(&mut fdc, &[0x1F]); // not a command
    assert_eq!(results(&mut fdc, 1), [0x80]);
}

#[test]
fn keyboard_reset_handshake_and_one_key_at_a_time() {
    let mut kbd = Keyboard::new();
    let (clock_low, clock_high) = (0x00u8, 0x40u8);
    kbd.port_b(clock_low, 0);
    kbd.port_b(clock_high, 120_000); // held low longer than 20 ms
    kbd.poll(clock_high, 120_500);
    assert!(!kbd.irq, "the self-test code takes a moment");
    kbd.poll(clock_high, 121_100);
    assert!(kbd.irq);
    assert_eq!(kbd.latch, 0xAA);
    kbd.push(0x1E);
    kbd.push(0x9E);
    kbd.poll(clock_high, 122_000);
    assert_eq!(kbd.latch, 0xAA, "the latch holds until acknowledged");
    kbd.port_b(clock_high | 0x80, 123_000); // acknowledge
    assert!(!kbd.irq);
    kbd.port_b(clock_high, 123_100);
    assert!(kbd.irq);
    assert_eq!(kbd.latch, 0x1E);
    kbd.port_b(clock_high | 0x80, 124_000);
    kbd.port_b(clock_high, 124_100);
    assert!(!kbd.irq, "scan codes are spaced about 5 ms apart");
    kbd.poll(clock_high, 149_000);
    assert!(kbd.irq);
    assert_eq!(kbd.latch, 0x9E);
}
