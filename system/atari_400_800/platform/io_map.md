# atari_400_800 I/O map

Hardware page $D000-$D4FF. Each chip repeats its registers through its
$200-byte slot: GTIA every 32 bytes ($D000-$D1FF), POKEY every 16
($D200-$D2FF), PIA every 4 ($D300-$D3FF), ANTIC every 16 ($D400-$D4FF).
Offsets below are within a chip's first 32 bytes.

## GTIA ($D000-$D1FF, 32-byte mirrors)

| Offset | Direction | Name | Behavior | Source |
|---|---|---|---|---|
| $00-$0F | W | HPOSP0-3, HPOSM0-3, SIZEP0-3, SIZEM, GRAFP0-3, GRAFM | stored; rendering not implemented yet | Tech. Ref. Notes |
| $00-$0F | R | M0PF-M3PF, P0PF-P3PF, M0PL-M3PL, P0PL-P3PL | collision registers; read 0 until P/M rendering exists | Tech. Ref. Notes |
| $10-$13 | R | TRIG0-TRIG3 | trigger inputs; 1 = released (no controllers attached) | Tech. Ref. Notes |
| $14 | R | PAL | NTSC machine reads $0F | Tech. Ref. Notes |
| $12-$1F | W | COLPM0-3, COLPF0-3, COLBK, PRIOR, VDELAY, GRACTL, HITCLR, CONSOL | stored for state dumps; HITCLR/CONSOL have no side effects yet | Tech. Ref. Notes |
| $1F | R | CONSOL | Start/Select/Option; $07 = all keys up | Tech. Ref. Notes |

## POKEY ($D200-$D2FF, 16-byte mirrors)

| Offset | Direction | Name | Behavior | Source |
|---|---|---|---|---|
| $00-$07 | W | AUDF1-4, AUDC1-4 | stored; audio output not implemented yet | Tech. Ref. Notes |
| $00-$07 | R | POT0-7 | paddle inputs; $FF (not connected) | Tech. Ref. Notes |
| $08 | W | AUDCTL | stored | Tech. Ref. Notes |
| $08 | R | ALLPOT | $FF (no pots pulled) | Tech. Ref. Notes |
| $09 | W | STIMER | no side effects yet (audio timers not modeled) | Tech. Ref. Notes |
| $09 | R | KBCODE | keyboard scan code; reset value $3F (A8-K8) | Tech. Ref. Notes |
| $0A | W | SKRES | no side effects yet | Tech. Ref. Notes |
| $0A | R | RANDOM | 17-bit poly counter, high 8 bits (A8-K7, TA-2) | Tech. Ref. Notes |
| $0B | W | POTGO | no side effects yet | Tech. Ref. Notes |
| $0D | W | SEROUT | stored nowhere; serial I/O not modeled | Tech. Ref. Notes |
| $0E | W | IRQEN | setting a bit 0->1 marks its IRQST bit pending (active low); handlers clear by rewriting IRQEN | Atari OS Manual, POKEY interrupts |
| $0E | R | IRQST | active-low status; reset $FF (A8-K6) | Tech. Ref. Notes |
| $0F | W | SKCTL | stored | Tech. Ref. Notes |
| $0F | R | SKSTAT | idle $EF: no serial activity, no key, no shift; bit 4 reads 0 (A8-K6) | Tech. Ref. Notes |

## PIA 6520 ($D300-$D3FF, 4-byte mirrors)

| Offset | Direction | Name | Behavior | Source |
|---|---|---|---|---|
| $00 | R/W | PORTA | joystick ports 1/2; control bit 2 selects DDR vs peripheral; inputs read $FF (released, A8-K9) | 6520 data sheet |
| $01 | R/W | PORTB | joystick ports 3/4 / memory control; same DDR semantics | 6520 data sheet |
| $02 | R/W | PACTL | port A control (bit 2 = DDR select) | 6520 data sheet |
| $03 | R/W | PBCTL | port B control (bit 2 = DDR select) | 6520 data sheet |

## ANTIC ($D400-$D4FF, 16-byte mirrors)

| Offset | Direction | Name | Behavior | Source |
|---|---|---|---|---|
| $00 | W | DMACTL | stored; DMA not implemented yet | Tech. Ref. Notes |
| $01 | W | CHACTL | stored | Tech. Ref. Notes |
| $02-$03 | W | DLISTL/DLISTH | stored | Tech. Ref. Notes |
| $04-$05 | W | HSCROL/VSCROL | stored | Tech. Ref. Notes |
| $04-$05 | R | PENH/PENV | light pen; reads $00 (A8-K5) | Tech. Ref. Notes |
| $07 | W | PMBASE | stored | Tech. Ref. Notes |
| $09 | W | CHBASE | stored | Tech. Ref. Notes |
| $0A | W | WSYNC | halts the CPU until the end of the current scanline (TA-1) | Tech. Ref. Notes |
| $0B | R | VCOUNT | current scanline / 2 | Tech. Ref. Notes |
| $0E | W | NMIEN | NMI enable: bit 7 DLI, bit 6 VBI | Tech. Ref. Notes |
| $0F | W | NMIRES | clears NMIST bits 7/6; re-arms the NMI edge | Tech. Ref. Notes |
| $0F | R | NMIST | bit 7 DLI, bit 6 VBI (set at start of line 248), bits 5/4 key/break never set in this slice | Tech. Ref. Notes |

## ASSUMPTION entries

Each entry is cross-referenced from code comments as `ASSUMPTION (spec A8-Kn)`
and validated in `emulator/plans/testplan.md`:

- **A8-K1** — The SALLY 6502C is modeled as the pinned NMOS 6502 core. The
  SALLY's HALT line (used by ANTIC/GTIA DMA) is not modeled; only WSYNC
  halts the CPU. Validation: run OS/BASIC titles once DMA exists; compare
  against the `ai_hw_engineer/design/mos_6502` RTL.
- **A8-K3** — Untouched regions ($C000-$CFFF, $D500-$D7FF) read $FF and
  writes are lost; RAM writes above the profile's RAM top are lost. Real
  floating-bus behavior varies. Validation: only OS-level behavior
  comparisons can detect a difference; revisit if a title depends on it.
- **A8-K4** — Write-only registers read $00 (except where the read map
  defines a value). Validation: OS boot behavior comparisons.
- **A8-K5** — Light pen PENH/PENV read $00. Validation: none until a light
  pen title is chosen; low risk.
- **A8-K6** — POKEY status constants: IRQST reset $FF, SKSTAT idle $EF with
  bit 4 always 0, keyboard/serial IRQ sources never fire in this slice.
  Validation: OS keyboard/serial paths once the input model lands.
- **A8-K7** — RANDOM is a 17-bit LFSR (reset $1FFFF, taps x^17+x^13, output
  = poly bits 17..10). The exact POKEY polynomial is not yet verified
  against an oracle; the sequence is therefore an approximation. Validation:
  compare RANDOM sequences against an independent emulator once pinned.
- **A8-K8** — KBCODE reset value $3F. Validation: OS keyboard init behavior.
- **A8-K9** — Paddle POT0-7/ALLPOT read $FF; PIA ports read $FF (joysticks
  released); CA2/CB2 handshake not modeled. Validation: joystick titles in
  the four-way matrix once the input model lands.
- **A8-K10** — Cartridges: 8 KiB at $A000 or 16 KiB at $8000; writes lost; no
  bank switching. Validation: run a bank-switched cart title and document the
  gap when it fails.
- **TA-1** — Instruction-granular scheduling: WSYNC takes effect at the end
  of the executing instruction; VBI fires at the start of line 248; ANTIC
  DMA cycle stealing is not modeled.
- **TA-2** — The POKEY poly counter advances once per bus access (the 6502
  performs one access per PHI2 cycle); cycle-exact alignment with real
  hardware is unverified.
- **TA-3** — The 8-cycle reset sequence is not charged to the cycle count.
