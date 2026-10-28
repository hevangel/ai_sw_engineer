; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Inline assembly with explicit register constraints: port I/O and the interrupt
; flag need no intrinsics.

; CHECK-LABEL: inb:
; CHECK:      mov dx, ax
; CHECK:      in al, dx
; CHECK:      ret
define i8 @inb(i16 %port) {
  %v = call i8 asm sideeffect "in al, dx", "={al},{dx}"(i16 %port)
  ret i8 %v
}

; CHECK-LABEL: outb:
; CHECK:      mov dx, ax
; CHECK:      mov al, cl
; CHECK:      out dx, al
define void @outb(i16 %port, i8 %v) {
  call void asm sideeffect "out dx, al", "{dx},{al}"(i16 %port, i8 %v)
  ret void
}

; CHECK-LABEL: flags:
; CHECK:      cli
; CHECK:      sti
define void @flags() {
  call void asm sideeffect "cli", ""()
  call void asm sideeffect "sti", ""()
  ret void
}
