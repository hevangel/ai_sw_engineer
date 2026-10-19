; busicom_141pf runtime -- HAL smoke program
; ============================================================================
; A hand-written 4004 program, linked with crt0.s + hal.s, that drives the
; Busicom board THROUGH the four HAL glue functions and produces an observable,
; deterministic result on the board's ports. It proves the glue path end to end
; on both simulators (run_image.sh secondary=chips, primary=RTL) without
; depending on the Rust firmware or the codegen backend.
;
; What it does, forever:
;   1. read the TEST pin via __bus_test_pin() and keep it in R10 (callee-saved
;      P5) so a corrupted glue return would show;
;   2. light the RAM1 lamp port (memory/overflow/negative lamps, io_map.md) with
;      the FIXED signature 0b0111 = 7 (all three named lamps on) -- a value the
;      original firmware never leaves set across a whole run, so seeing LAMPS|7
;      in the trace is an unambiguous "my code drove RAM1" signal;
;   3. write the RAM0 output port = 0 (exercises the second 4002's write path;
;      0 means no print / no paper advance, so it cannot forge a printed row);
;   4. loop.
;
; The runner samples RAM1 (LAMPS) once per step; the smoke test passes if the
; captured LAMPS values are 7 (our signature). __bus_test_pin is exercised too
; (its result is read) and is unit-tested for BOTH pin levels on the bare-CPU
; harness (tests/run_hal_tests.sh), where the pin is controllable.
;
; main() is called by crt0 after startup; it never returns.
	.text
	.globl	main
main:
.Lloop:
	; --- read TEST pin (exercise the glue read path) ---
	jms	__bus_test_pin		; P1.lo (R3) = 1 if HIGH else 0
	ld	r3
	xch	r10			; keep the reading in P5 (callee-saved)

	; --- light RAM1 lamps with the fixed signature 7 ---
	fim	p1, 0x01		; chip = RAM1 (select value 1 -> SRC[7:6]=1)
	fim	p2, 0x07		; value = 0b0111 (all three named lamps)
	jms	__bus_ram_port_write

	; --- exercise the RAM0 output write path (clear it) ---
	fim	p1, 0x00		; chip = RAM0
	fim	p2, 0x00		; value = 0 (no print, no paper advance)
	jms	__bus_ram_port_write

	jun	.Lloop
