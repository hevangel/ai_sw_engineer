; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i8 add over two nibbles via the accumulator, with carry propagation
; (abi.md sec.2; the 4004 has a single 4-bit ACC and no reg-reg ALU, so an
; 8-bit add becomes CLC; LD; ADD; XCH per nibble, low nibble first).
; Args a,b in P1 (r2:r3) and P2 (r4:r5); result in P1 (abi.md sec.4.1/4.2).

; CHECK-LABEL: add8:
; CHECK:      clc
; CHECK-NEXT: ld r3
; CHECK-NEXT: add r5
; CHECK-NEXT: xch r3
; CHECK-NEXT: ld r2
; CHECK-NEXT: add r4
; CHECK-NEXT: xch r2
; CHECK-NEXT: bbl 0
define i8 @add8(i8 %a, i8 %b) {
  %r = add i8 %a, %b
  ret i8 %r
}
