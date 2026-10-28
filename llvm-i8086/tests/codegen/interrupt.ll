; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Interrupt handlers (spec/abi.md D5): the "interrupt" attribute, or the
; msp430_intrcc calling convention that rustc uses for extern "msp430-interrupt".

@ticks = global i16 0

declare void @other()

; Every register is saved, DS and ES too, and DS and ES are set to CS; the
; handler ends with IRET.
; CHECK-LABEL: tick:
; CHECK:      push bp
; CHECK-NEXT: mov bp, sp
; CHECK-NEXT: push ax
; CHECK:      push ds
; CHECK-NEXT: push es
; CHECK-NEXT: push cs
; CHECK-NEXT: pop ds
; CHECK-NEXT: push cs
; CHECK-NEXT: pop es
; CHECK-NEXT: cld
; CHECK:      add ax, word ptr [ticks]
; CHECK:      lea sp, [bp-0x10]
; CHECK-NEXT: pop es
; CHECK-NEXT: pop ds
; CHECK:      pop bp
; CHECK-NEXT: iret
define void @tick() #0 {
  %v = load i16, ptr @ticks
  %n = add i16 %v, 1
  store i16 %n, ptr @ticks
  ret void
}

; A handler that calls: the caller-saved registers were saved too.
; CHECK-LABEL: busy:
; CHECK:      push ax
; CHECK:      push cx
; CHECK:      push dx
; CHECK:      call other
; CHECK:      iret
define void @busy() #0 {
  call void @other()
  ret void
}

; The calling convention rustc gives extern "msp430-interrupt".
; CHECK-LABEL: by_cc:
; CHECK:      push ax
; CHECK:      iret
define msp430_intrcc void @by_cc() {
  %v = load i16, ptr @ticks
  %n = add i16 %v, 2
  store i16 %n, ptr @ticks
  ret void
}

; An ordinary function is unchanged.
; CHECK-LABEL: plain:
; CHECK-NOT:  iret
; CHECK:      ret
define void @plain() {
  store i16 7, ptr @ticks
  ret void
}

attributes #0 = { "interrupt" }
