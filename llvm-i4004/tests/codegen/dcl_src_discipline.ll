; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; DCL / SRC discipline (abi.md sec.4.4; failure_notes/2026-10-19-chips-4002-src-
; latch-ignores-dcl.md). A per-function frame lives in a CM-RAM bank selected by
; DCL; every frame access brackets itself `FIM __framebank; DCL; FIM __frame;
; SRC; <I/O>; ... ; LDM 0; DCL`. The 4002 SRC address latch is PER-CHIP and
; gated by the CM-RAM (DCL) bank line, so an SRC issued under one DCL does NOT
; latch the chip that answers under a different DCL. I4004Peephole must therefore
; treat a DCL (and any JMS/JUN/JIN/BBL/inline-asm) as INVALIDATING its tracked
; SRC latch: a `SRC` that follows a `DCL` must never be elided as "already
; latched". This test pins that the bracket structure survives the peephole --
; every RAM I/O (wrm/rdm) is preceded, since the last dcl, by a fresh src, and
; no `dcl` is immediately followed by a RAM I/O with no intervening `src`.
;
; `f` spills `%y` across the call to @ext (forcing a per-function frame store),
; then reloads it after the call (a frame load). Each bracket must re-SRC.

declare i8 @ext(i8)

define i8 @f(i8 %x, i8 %y) {
  %c = call i8 @ext(i8 %x)
  %r = add i8 %c, %y
  ret i8 %r
}

; The SPILL bracket: select frame bank, load frame base, SRC, then I/O.
; CHECK-LABEL: f:
; CHECK:      fim p0, __framebank_f
; CHECK-NEXT: ld r1
; CHECK-NEXT: dcl
; CHECK-NEXT: fim p0, __frame_f
; The DCL above must NOT cause the following SRC to be elided.
; CHECK-NEXT: src p0
; CHECK:      wrm
; second nibble: pointer advanced (inc), so a fresh SRC is required and kept.
; CHECK:      inc r1
; CHECK-NEXT: src p0
; CHECK:      wrm
; bracket closes by restoring DCL 0.
; CHECK:      ldm 0
; CHECK-NEXT: dcl

; The RELOAD bracket after the call: a fresh DCL + SRC again (the call clobbered
; both the SRC latch and the DCL bank; the peephole must not carry either across
; the JMS/JUN soft-call sequence).
; CHECK:      fim p0, __framebank_f
; CHECK-NEXT: ld r1
; CHECK-NEXT: dcl
; CHECK-NEXT: fim p0, __frame_f
; CHECK-NEXT: src p0
; CHECK-NEXT: rdm
; CHECK:      inc r1
; CHECK-NEXT: src p0
; CHECK-NEXT: rdm
; CHECK:      ldm 0
; CHECK-NEXT: dcl
; CHECK-NEXT: jun __soft_ret
