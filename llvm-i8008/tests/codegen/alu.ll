; RUN: llc -mtriple=i8008 -O2 %s -o - | FileCheck %s
; Register ALU: through A (codegen.md sec.2-3; abi.md sec.3 argument registers).

; CHECK-LABEL: add8:
; CHECK-NEXT: ; %bb.0:
; CHECK-NEXT: lab
; CHECK-NEXT: adc
; CHECK-NEXT: lba
; CHECK-NEXT: ret
define i8 @add8(i8 %a, i8 %b) {
  %r = add i8 %a, %b
  ret i8 %r
}

; CHECK-LABEL: add16:
; CHECK: lac
; CHECK-NEXT: ade
; CHECK-NEXT: lca
; CHECK-NEXT: lab
; CHECK-NEXT: acd
; CHECK-NEXT: lba
; CHECK-NEXT: ret
define i16 @add16(i16 %a, i16 %b) {
  %r = add i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: sub8i:
; CHECK: lab
; CHECK-NEXT: adi 0xfb
; CHECK-NEXT: lba
define i8 @sub8i(i8 %a) {
  %r = sub i8 %a, 5
  ret i8 %r
}

; CHECK-LABEL: inc8:
; CHECK: inb
; CHECK-NEXT: ret
define i8 @inc8(i8 %a) {
  %r = add i8 %a, 1
  ret i8 %r
}

; CHECK-LABEL: and16i:
; CHECK: lac
; CHECK-NEXT: ndi 0x0f
; CHECK-NEXT: lca
; CHECK-NEXT: lbi 0x00
define i16 @and16i(i16 %a) {
  %r = and i16 %a, 15
  ret i16 %r
}

; CHECK-LABEL: rsub8:
; CHECK: lai 0x07
; CHECK-NEXT: sub
; CHECK-NEXT: lba
define i8 @rsub8(i8 %a) {
  %r = sub i8 7, %a
  ret i8 %r
}

; CHECK-LABEL: sra2:
; CHECK: lab
; CHECK-NEXT: rlc
; CHECK-NEXT: rrc
; CHECK-NEXT: rar
; CHECK-NEXT: rlc
; CHECK-NEXT: rrc
; CHECK-NEXT: rar
; CHECK-NEXT: lba
define i8 @sra2(i8 %a) {
  %r = ashr i8 %a, 2
  ret i8 %r
}

; CHECK-LABEL: srl3:
; CHECK: lab
; CHECK-NEXT: rrc
; CHECK-NEXT: rrc
; CHECK-NEXT: rrc
; CHECK-NEXT: ndi 0x1f
; CHECK-NEXT: lba
define i8 @srl3(i8 %a) {
  %r = lshr i8 %a, 3
  ret i8 %r
}

; CHECK-LABEL: shl16by9:
; CHECK: lac
; CHECK-NEXT: ada
; CHECK-NEXT: lba
; CHECK-NEXT: lci 0x00
define i16 @shl16by9(i16 %a) {
  %r = shl i16 %a, 9
  ret i16 %r
}

; CHECK-LABEL: sext:
; CHECK: lab
; CHECK-NEXT: lca
; CHECK-NEXT: rlc
; CHECK-NEXT: sba
; CHECK-NEXT: lba
define i16 @sext(i8 %a) {
  %r = sext i8 %a to i16
  ret i16 %r
}

; Variable shifts and multiplies are runtime calls (abi.md sec.6).
; CHECK-LABEL: shlv:
; CHECK: cal __ashlqi3
define i8 @shlv(i8 %a, i8 %n) {
  %r = shl i8 %a, %n
  ret i8 %r
}
; CHECK-LABEL: mul16:
; CHECK: cal __mulhi3
define i16 @mul16(i16 %a, i16 %b) {
  %r = mul i16 %a, %b
  ret i16 %r
}
