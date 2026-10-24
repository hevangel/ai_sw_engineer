# Micral N I/O map

The 8008's I/O cycle puts A on address bits 0–7 and the instruction's port
field on bits 9–13 (`isa.md`, "What the Micral N adds"). A Micral card decodes
both, so a device is reached by an `INP p` or `OUT p` **and**, for inputs, a
sub-address in A. Every title (and the simulators in `sim/`) uses the
configuration of the machine MO5 restored. R2E let the user strap each card's
address, so this is one installation, not "the" Micral N.

Sources:

- [R2E] *MICRAL N User's Manual*, January 1974, chapter II "MIC 01 Micral
  Monitor", §II.2 (`../../../../ai_hw_engineer/system/micral_n/spec/reference/MICRAL_N_Users_Manual_Jan74.md`).
- [HxC] J.-F. Del Nero, *Restauration d'un Micral N*, card table
  (hxc2001.pages-perso.free.fr/micral-n/), read 2026-10-23.
- [MO5emu] MO5's *Virtual Micral N* (MIT licence, github.com/Asso-MO5/virtual_micral_n),
  `src/devices/src/{IOCard,SerialCard,StackChannelCard,DiskControllerCard,MemoryCard}.cpp`
  and `src/emulator/src/{Simulator,VirtualDisk}.cpp`. The emulator's authors
  built these models from the real cards and boot ROM, and they mark parts as
  guesses. Where we follow such a guess, an ASSUMPTION row says so.

## Teletype (asynchronous card behind the 32-32 card)

| Instruction | A in | Result | Source |
|---|---|---|---|
| `INP 0` | 0 | received character; reading it clears "input ready" | [R2E] §II.2.4 (input interface 0), [MO5emu] IOCard input 0, SerialCard |
| `INP 0` | 1 | status: bit 0 input ready, bit 4 stop-bit error, bit 5 parity error, bit 6 framing error, bit 7 output ready | [R2E] §II.2.4 status table, [MO5emu] SerialCard `combined_status` |
| `OUT 8` | char | send one character | [R2E] §II.2.4 (output interface 0), [MO5emu] IOCard output 0 → SerialCard |

The 32-32 card answers group-0 inputs only when A bit 2 is 0, so A = 0 and
A = 1 select the two teletype inputs.

- ASSUMPTION (platform IO-1): the line is 110 baud, 8 bits, no parity (the
  [HxC] card table, "réglée en 110 Bauds, 8 Bits"). The simulators deliver
  characters with no baud-rate delay. Programs must poll the status bit, never
  count cycles.
- ASSUMPTION (platform IO-2): software sends 7-bit ASCII with bit 7 clear and
  ignores bit 7 on input. [HxC] notes that the monitor emulates 7-bit + parity
  in software; our programs neither generate nor check parity. Validated by:
  the MO5 emulator's CLI masks input to 7 bits.
- Carriage return (0x0D) ends a line. Programs print CR LF.

## Stack card ("pile canal", stack mode)

| Instruction | Result | Source |
|---|---|---|
| `INP 6` | pop: pointer − 1 (mod 256), return the byte there | [R2E] §II.2.3 "addresses 6 for input and 22 for output", [MO5emu] StackChannelCard (`input_address = 6`, `output_address = 0x16`) |
| `OUT 30` | push A: store at pointer, pointer + 1 (mod 256) | same; output address 0x16 (22) is `OUT 30` because output ports start at 8 |

A 256-byte LIFO in its own memory. The monitor uses it ([R2E] §II.2.3, [HxC]).

## Floppy disk (formatter-coupler card + stack card in channel mode)

The disk controller fills the channel card's buffer; the CPU then pops the
bytes. The boot ROM is the only user.

| Instruction | A | Effect | Source |
|---|---|---|---|
| `OUT 23` | command | bits 0–4 sector; bit 5 DIR (1 = step towards higher tracks); bit 6 STEP (a 1→0 change steps one track) | [MO5emu] DiskControllerCard `on_command` |
| `OUT 20` | pointer | channel buffer pointer | `on_set_pointer` |
| `OUT 21` | count | bytes to transfer (the channel takes one more) | `on_set_counter`, StackChannelCard `on_apply_counter` |
| `OUT 22` | — | start the transfer, disk → channel | `on_activate` |
| `INP 0` | 0xFE | status: bit 7 head on track 0, bit 1 reading, bit 0 transferring | `update_card_status` |
| `INP 0` | 0xFF | channel pointer (low byte) | connector `current_pointer_address` |
| `INP 5` | — | pop a byte from the channel buffer (LIFO: the last byte read comes out first) | StackChannelCard `pop_data` |

Sector layout as the controller delivers it: two leader bytes
(0xFF, 0xFE), 128 data bytes, one checksum byte, then filler
([MO5emu] VirtualDisk `get`). The checksum adds each data byte with an
end-around carry taken from the signed overflow of the addition
(`computer_sector_checksum`).

- ASSUMPTION (platform IO-3): the transfer protocol, status bits and LIFO
  order are [MO5emu]'s reconstruction (its source says "tentative" and
  "probably"). Our boot ROM and both simulators use it, so they agree with
  each other. A dump of the real boot ROM would be the oracle for it.
- ASSUMPTION (platform IO-4): the disk has 64 tracks of 32 × 128-byte sectors
  (256 KiB, [HxC]). The image the boot ROM loads starts at track 0, sector 16
  ([MO5emu] `create_virtual_disk` puts 16 filler sectors first).

## Boot ROM switch

The 4 KiB RAM card's 256-byte EPROM answers reads of page 0 until the CPU
fetches an `RST 0` instruction. From then on page 0 reads come from RAM. Writes
to page 0 always go to RAM ([MO5emu] MemoryCard `ROM_RAM_256`, `on_phase_2`).
At power-on the boot ROM can therefore copy a program over page 0 and start it
with `RST 0`. MO5 describe the boot as loading the first 14 KiB of the disk to
address 0, checking checksums, then restarting with the ROM switched off
(MO5 association articles "Programmer le Micral N" and "Lecture et sauvegarde
des ROMs du Micral N", summarised in search results 2026-10-23; the pages now
return 404).

## Console

The front panel can substitute instruction bytes. To start the monitor, the
operator feeds `JMP 3B30` (0x44 0x30 0x3B) ([R2E] §II.3.1, [HxC] "Démarrage du
moniteur"). The simulators' `--start ADDR` does the same at reset.
