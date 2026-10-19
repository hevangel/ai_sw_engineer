; busicom_141pf runtime -- board HAL glue (abi.md C calling convention)
; ============================================================================
; The FOUR C-ABI functions the Rust target HAL (system/busicom_141pf/firmware
; src/main.rs via the platform HAL) calls to reach the MCS-4 bus. This is a
; FIXED interface -- the firmware agent codes against exactly these symbols and
; signatures -- so changing a name or argument order here breaks the firmware
; build. Arguments follow abi.md sec.4.1 (P1,P2,...); i8 returns in P1 (sec.4.2).
;
; Chip-select and port semantics are from platform/io_map.md and isa.md sec.5:
;   * A ROM I/O port (4001) is addressed by SRC[7:4] = ROM chip index; WRR
;     writes ACC to it, RDR reads it into ACC. SRC[3:0] is ignored by the port.
;   * A RAM output port (4002) is addressed by SRC[7:4] = 4002 chip index (RAM0
;     is P0=0, RAM1 is P0=1 per io_map.md) within the DCL-selected bank; WMP
;     writes ACC to it. The Busicom board uses CM-RAM bank 0.
;   * The TEST pin is sampled by JCN condition bit C4: the jump is taken when
;     TEST is LOW (platform/README.md "JCN TEST-pin polarity"; the MCS-4 manual
;     and the ai_hw_engineer RTL agree -- external/chips uses the inverse
;     convention internally, which the oracle capture compensates for).
;
; An argument's 8-bit value is in a pair {Reven:hi, Rodd:lo}. The chip index is
; a small number in the arg's LOW nibble (Rodd). To drive SRC we need it in the
; pointer pair's HIGH nibble, so each function moves the chip nibble into the
; even register of P0 and zeroes P0's low nibble.
;
; These are leaf functions (one bus op each) and clobber only ACC, CY and P0;
; callee-saved P4-P6 and the SSP P7 are untouched, so the Rust caller's state
; survives.  Sizes are reported in runtime/README.md.

; ---------------------------------------------------------------------------
; void __bus_rom_port_write(u8 chip /*P1*/, u8 value /*P2*/)
;   SRC{chip<<4} ; ACC = value.lo ; WRR
; ---------------------------------------------------------------------------
	.section .text.__bus_rom_port_write,"ax"
	.globl	__bus_rom_port_write
__bus_rom_port_write:
	ld	r3			; chip index (P1 low nibble)
	xch	r0			; P0 high nibble (R0) = chip  -> SRC[7:4]
	ldm	0
	xch	r1			; P0 low nibble (R1) = 0      -> SRC[3:0] (ignored)
	src	p0			; latch ROM-port address = chip<<4
	ld	r5			; value (P2 low nibble -> the 4-bit ROM port)
	wrr				; ROM I/O port <- ACC
	bbl	0

; ---------------------------------------------------------------------------
; u8 __bus_rom_port_read(u8 chip /*P1*/) -> value in P1 low nibble
;   SRC{chip<<4} ; RDR ; result -> P1.lo (R3). (BBL clobbers ACC, so the i8
;   return is materialised in P1 per abi.md sec.4.2.)
; ---------------------------------------------------------------------------
	.section .text.__bus_rom_port_read,"ax"
	.globl	__bus_rom_port_read
__bus_rom_port_read:
	ld	r3
	xch	r0			; P0.hi = chip
	ldm	0
	xch	r1			; P0.lo = 0
	src	p0
	rdr				; ACC <- ROM I/O port (low 4 bits)
	xch	r3			; P1.lo = result
	ldm	0
	xch	r2			; P1.hi = 0  (zero-extend the u8)
	bbl	0

; ---------------------------------------------------------------------------
; void __bus_ram_port_write(u8 chip /*P1*/, u8 value /*P2*/)
;   DCL bank 0 ; SRC{chip in [7:6]} ; ACC = value.lo ; WMP
;   4002 chip select is SRC bits [7:6] (isa.md sec.5: RAM pointer [7:6] selects
;   the chip, [5:4] the register, [3:0] the char). The chip index must therefore
;   sit in the TOP two bits of the SRC byte = (chip << 2) in the high nibble.
;   RAM0 = chip 0 -> high nibble 0x0 ; RAM1 = chip 1 -> high nibble 0x4.
; ---------------------------------------------------------------------------
	.section .text.__bus_ram_port_write,"ax"
	.globl	__bus_ram_port_write
__bus_ram_port_write:
	ldm	0
	dcl				; select CM-RAM bank 0 (Busicom board)
	; build SRC high nibble = chip << 2 (two left rotates of a byte with 0 fill)
	ld	r3			; chip index (0..3) in low 2 bits
	clc
	ral				; chip << 1 (bit3<-0 since chip<=3, CY irrelevant)
	clc
	ral				; chip << 2  -> now in bits [3:2] of the nibble = SRC[7:6]
	xch	r0			; P0.hi = chip<<2  -> SRC[7:6] selects the 4002
	ldm	0
	xch	r1			; P0.lo = 0  (reg/char irrelevant for the output port)
	src	p0
	ld	r5			; value (P2 low nibble -> 4-bit RAM output port)
	wmp				; RAM output port <- ACC
	bbl	0

; ---------------------------------------------------------------------------
; u8 __bus_test_pin(void) -> 1 if TEST pin is HIGH, else 0 (P1 low nibble)
;   JCN C4 jumps when TEST is LOW (platform/README.md). So: assume HIGH (1);
;   if the "jump when TEST low" branch is taken, overwrite with 0.
; ---------------------------------------------------------------------------
	.section .text.__bus_test_pin,"ax"
	.globl	__bus_test_pin
__bus_test_pin:
	ldm	1			; tentative result = 1 (TEST HIGH)
	xch	r3			; P1.lo = 1
	jcn	t, .Ltest_low		; T = cond 0x1 (C4): jump when TEST is LOW
	jun	.Ltest_done		; not taken -> TEST was HIGH -> keep 1
.Ltest_low:
	ldm	0
	xch	r3			; TEST LOW -> result = 0
.Ltest_done:
	ldm	0
	xch	r2			; P1.hi = 0 (zero-extend)
	bbl	0
