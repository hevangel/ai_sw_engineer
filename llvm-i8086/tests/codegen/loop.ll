; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; `dec cx ; jne` is `loop` in a function small enough for its 8-bit reach; the
; test that repeats the flags of an arithmetic instruction is dropped everywhere.

; CHECK-LABEL: sum_down:
; CHECK:      mov cx, dx
; CHECK:      loop {{\.LBB[0-9_]+}}
; CHECK-NOT:  dec
; CHECK-NOT:  jne
define i16 @sum_down(ptr %p, i16 %n) {
entry:
  br label %l
l:
  %i = phi i16 [ %n, %entry ], [ %i2, %l ]
  %acc = phi i16 [ 0, %entry ], [ %a2, %l ]
  %q = getelementptr i16, ptr %p, i16 %i
  %v = load i16, ptr %q
  %a2 = add i16 %acc, %v
  %i2 = sub i16 %i, 1
  %c = icmp ne i16 %i2, 0
  br i1 %c, label %l, label %out
out:
  ret i16 %a2
}

; The flags of `sub` already say whether the difference is zero.
; CHECK-LABEL: same:
; CHECK:      sub
; CHECK-NOT:  test
; CHECK:      je
define i16 @same(i16 %a, i16 %b) {
  %d = sub i16 %a, %b
  %c = icmp eq i16 %d, 0
  br i1 %c, label %z, label %nz
z:
  ret i16 1
nz:
  ret i16 %d
}
