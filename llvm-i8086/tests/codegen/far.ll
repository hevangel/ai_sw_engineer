; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Far data pointers (address space 1): a load is ES loaded with the segment and
; an access through ES:[bx], with ES saved and restored.

; CHECK-LABEL: load16:
; CHECK:      push es
; CHECK-NEXT: mov es, {{[a-d]x}}
; CHECK-NEXT: mov {{[a-d]x}}, word ptr es:[bx]
; CHECK-NEXT: pop es
define i16 @load16(ptr addrspace(1) %p) {
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}

; CHECK-LABEL: store8:
; CHECK:      push es
; CHECK-NEXT: mov es, {{[a-d]x}}
; CHECK-NEXT: mov byte ptr es:[bx], {{[a-d]l}}
; CHECK-NEXT: pop es
define void @store8(ptr addrspace(1) %p, i8 %v) {
  store i8 %v, ptr addrspace(1) %p
  ret void
}

; A 32-bit load is two word accesses, at offsets 0 and 2.
; CHECK-LABEL: load32:
; CHECK:      word ptr es:[bx]
; CHECK:      word ptr es:[bx]
define i32 @load32(ptr addrspace(1) %p) {
  %v = load i32, ptr addrspace(1) %p
  ret i32 %v
}

; Pointer arithmetic changes the offset only: no carry into the segment.
; CHECK-LABEL: elem:
; CHECK-NOT:  adc
define ptr addrspace(1) @elem(ptr addrspace(1) %p, i16 %i) {
  %q = getelementptr i16, ptr addrspace(1) %p, i16 %i
  ret ptr addrspace(1) %q
}
