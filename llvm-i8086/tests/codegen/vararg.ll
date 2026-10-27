; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Variadic functions and byval arguments (spec/abi.md D3a).

declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
declare i16 @sum(i16, ...)

; va_start: the list points at the first stack argument after the named ones,
; which are on the stack too. With one named argument ([bp+4]) that is [bp+6].
; CHECK-LABEL: first_vararg:
; CHECK:      lea ax, [bp+6]
; CHECK:      mov word ptr [bp-
define i16 @first_vararg(i16 %n, ...) {
  %ap = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  %p = load ptr, ptr %ap
  %v = load i16, ptr %p
  call void @llvm.va_end.p0(ptr %ap)
  ret i16 %v
}

; va_arg: load through the list and advance it by the slot size (2 bytes each;
; the optimizer folds the two advances into one).
; CHECK-LABEL: next_vararg:
; CHECK:      add {{[a-z]+}}, 4
; CHECK:      word ptr [bp+6]
; CHECK:      word ptr [bp+8]
define i16 @next_vararg(i16 %n, ...) {
  %ap = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  %a = va_arg ptr %ap, i16
  %b = va_arg ptr %ap, i16
  call void @llvm.va_end.p0(ptr %ap)
  %r = add i16 %a, %b
  ret i16 %r
}

; A call to a variadic function passes every argument on the stack, the fixed
; one (the count) last of the pushes because it is the first argument.
; CHECK-LABEL: call_variadic:
; CHECK:      push ax
; CHECK:      push
; CHECK:      call sum
; CHECK-NEXT: add sp, 6
define i16 @call_variadic(i16 %x, i16 %y) {
  %r = call i16 (i16, ...) @sum(i16 2, i16 %x, i16 %y)
  ret i16 %r
}

; byval: the object is copied onto the stack a word at a time, highest first.
%S = type { i16, i16, i16 }
declare void @take(ptr byval(%S) align 2)

; CHECK-LABEL: pass_byval:
; CHECK:      mov ax, word ptr [{{[a-z]+}}+4]
; CHECK-NEXT: push ax
; CHECK-NEXT: mov ax, word ptr [{{[a-z]+}}+2]
; CHECK-NEXT: push ax
; CHECK-NEXT: mov ax, word ptr [{{[a-z]+}}]
; CHECK-NEXT: push ax
; CHECK-NEXT: call take
; CHECK-NEXT: add sp, 6
define void @pass_byval(ptr %p) {
  call void @take(ptr byval(%S) align 2 %p)
  ret void
}

; In the callee a byval argument is the stack memory itself.
; CHECK-LABEL: read_byval:
; CHECK:      word ptr [bp+8]
define i16 @read_byval(ptr byval(%S) align 2 %p) {
  %q = getelementptr inbounds %S, ptr %p, i16 0, i32 2
  %v = load i16, ptr %q
  ret i16 %v
}
