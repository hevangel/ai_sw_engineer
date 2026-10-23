; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i16 arithmetic. i16 is legal in a DR16 double-pair (AVR DREGS analog) but the
; add/sub are expanded to a four-nibble ACC chain over the two sub-pairs, carry
; propagating low->high (abi.md sec.1.2: i16 is not a native width).

; CHECK-LABEL: add16:
; CHECK:      clc
; CHECK-COUNT-4: add r{{[0-9]+}}
; CHECK:      bbl 0
define i16 @add16(i16 %x, i16 %y) {
  %r = add i16 %x, %y
  ret i16 %r
}

; CHECK-LABEL: sub16:
; CHECK:      clc
; CHECK-COUNT-4: sub r{{[0-9]+}}
; CHECK:      bbl 0
define i16 @sub16(i16 %x, i16 %y) {
  %r = sub i16 %x, %y
  ret i16 %r
}
