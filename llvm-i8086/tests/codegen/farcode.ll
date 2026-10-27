; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Far calls: `push cs ; call` to a function in the segment, `call far [slot]` through
; a pointer; a far function returns with retf.

; CHECK-LABEL: callee:
; CHECK:      retf
define cc 1000 i16 @callee(i16 %a) {
  %r = add i16 %a, 1
  ret i16 %r
}

; CHECK-LABEL: direct:
; CHECK:      call
; CHECK-SAME: callee@seg:callee
define i16 @direct(i16 %a) {
  %r = call cc 1000 i16 @callee(i16 %a)
  ret i16 %r
}

; CHECK-LABEL: indirect:
; CHECK:      call dword ptr [bp
define i16 @indirect(ptr addrspace(1) %f, i16 %a) {
  %r = call cc 1000 addrspace(1) i16 %f(i16 %a)
  ret i16 %r
}

; stack arguments of a far function start four bytes above bp
; CHECK-LABEL: stack_args:
; CHECK:      [bp+6]
define cc 1000 i16 @stack_args(i16 %a, i16 %b, i16 %c, i16 %d) {
  %s = add i16 %a, %d
  ret i16 %s
}
