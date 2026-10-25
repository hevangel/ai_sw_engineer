; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Addressing modes and the frame (spec/codegen.md, Memory operands).

@counter = global i16 0
@arr = global [8 x i16] zeroinitializer

; An absolute address with AX is the short accumulator form.
; CHECK-LABEL: get:
; CHECK:      mov ax, word ptr [counter]
; CHECK-NEXT: ret
define i16 @get() {
  %v = load i16, ptr @counter
  ret i16 %v
}

; CHECK-LABEL: set:
; CHECK:      mov word ptr [counter], ax
; CHECK-NEXT: ret
define void @set(i16 %v) {
  store i16 %v, ptr @counter
  ret void
}

; A load folded into the ALU instruction.
; CHECK-LABEL: bump:
; CHECK:      add ax, word ptr [counter]
; CHECK-NEXT: mov word ptr [counter], ax
define i16 @bump(i16 %by) {
  %v = load i16, ptr @counter
  %n = add i16 %v, %by
  store i16 %n, ptr @counter
  ret i16 %n
}

; Symbol plus constant, and symbol plus register (BX is the base).
; CHECK-LABEL: elem5:
; CHECK:      mov ax, word ptr [arr+10]
define i16 @elem5() {
  %p = getelementptr i16, ptr @arr, i16 5
  %v = load i16, ptr %p
  ret i16 %v
}

; CHECK-LABEL: elem:
; CHECK:      push bx
; CHECK:      shl bx, 1
; CHECK-NEXT: mov ax, word ptr [bx+arr]
; CHECK-NEXT: pop bx
define i16 @elem(i16 %i) {
  %p = getelementptr i16, ptr @arr, i16 %i
  %v = load i16, ptr %p
  ret i16 %v
}

; A pointer argument plus a constant.
; CHECK-LABEL: field:
; CHECK:      mov bx, ax
; CHECK-NEXT: mov ax, word ptr [bx+4]
define i16 @field(ptr %s) {
  %f = getelementptr i16, ptr %s, i16 2
  %v = load i16, ptr %f
  ret i16 %v
}

; Base plus index: [bx+si].
; CHECK-LABEL: fill:
; CHECK:      mov byte ptr [bx+si], cl
define void @fill(ptr %p, i16 %n, i8 %v) {
entry:
  %empty = icmp eq i16 %n, 0
  br i1 %empty, label %exit, label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %q = getelementptr i8, ptr %p, i16 %i
  store i8 %v, ptr %q
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, %n
  br i1 %more, label %loop, label %exit
exit:
  ret void
}

; A stack object: frame pointer, locals below the saved registers, indexed
; with [bp+si+disp], and an epilogue that restores SP from BP.
; CHECK-LABEL: local:
; CHECK:      push bp
; CHECK-NEXT: mov bp, sp
; CHECK-NEXT: push si
; CHECK-NEXT: sub sp, 8
; CHECK:      mov word ptr [bp-4], 0x190
; CHECK:      mov ax, word ptr [bp+si-0xa]
; CHECK-NEXT: lea sp, [bp-2]
; CHECK-NEXT: pop si
; CHECK-NEXT: pop bp
; CHECK-NEXT: ret
define i16 @local(i16 %i) {
  %t = alloca [4 x i16], align 2
  store i16 100, ptr %t
  %p1 = getelementptr i16, ptr %t, i16 1
  store i16 200, ptr %p1
  %p2 = getelementptr i16, ptr %t, i16 2
  store i16 300, ptr %p2
  %p3 = getelementptr i16, ptr %t, i16 3
  store i16 400, ptr %p3
  %k = and i16 %i, 3
  %q = getelementptr i16, ptr %t, i16 %k
  %v = load i16, ptr %q
  ret i16 %v
}
