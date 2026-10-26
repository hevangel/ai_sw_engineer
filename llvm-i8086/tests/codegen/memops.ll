; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; rep movs and rep stos (spec/codegen.md, Memory operations).

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg)
declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg)

; A copy of run-time length: CX = count, DI = destination, SI = source.
; CHECK-LABEL: copy:
; CHECK:      mov di, ax
; CHECK-NEXT: mov si, dx
; CHECK-NEXT: rep
; CHECK-NEXT: movsb
define void @copy(ptr %d, ptr %s, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}

; An even, aligned size is copied a word at a time.
; CHECK-LABEL: copy64:
; CHECK:      mov cx, 0x20
; CHECK:      rep
; CHECK-NEXT: movsw
define void @copy64(ptr align 2 %d, ptr align 2 %s) {
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %d, ptr align 2 %s, i16 64, i1 false)
  ret void
}

; A small copy is loads and stores.
; CHECK-LABEL: copy6:
; CHECK-NOT:  rep
; CHECK:      ret
define void @copy6(ptr align 2 %d, ptr align 2 %s) {
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %d, ptr align 2 %s, i16 6, i1 false)
  ret void
}

; CHECK-LABEL: fill:
; CHECK:      mov al, dl
; CHECK-NEXT: rep
; CHECK-NEXT: stosb
define void @fill(ptr %d, i8 %v, i16 %n) {
  call void @llvm.memset.p0.i16(ptr %d, i8 %v, i16 %n, i1 false)
  ret void
}

; A zero fill of 128 aligned bytes is 64 words.
; CHECK-LABEL: zero128:
; CHECK:      mov cx, 0x40
; CHECK:      mov ax, 0
; CHECK-NEXT: rep
; CHECK-NEXT: stosw
define void @zero128(ptr align 2 %d) {
  call void @llvm.memset.p0.i16(ptr align 2 %d, i8 0, i16 128, i1 false)
  ret void
}

; A constant byte in a byte fill.
; CHECK-LABEL: pattern50:
; CHECK:      mov al, 0xa5
; CHECK-NEXT: rep
; CHECK-NEXT: stosb
define void @pattern50(ptr %d) {
  call void @llvm.memset.p0.i16(ptr %d, i8 165, i16 50, i1 false)
  ret void
}
