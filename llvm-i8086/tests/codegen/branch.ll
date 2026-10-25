; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Comparisons, branches and selects (spec/codegen.md, Control flow).

; A compare with a register; the block laid out next is the fall-through.
; CHECK-LABEL: cmp_branch:
; CHECK:      cmp ax, dx
; CHECK-NEXT: jae
; CHECK:      ret
; CHECK:      mov ax, dx
; CHECK-NEXT: ret
define i16 @cmp_branch(i16 %a, i16 %b) {
  %c = icmp ult i16 %a, %b
  br i1 %c, label %lt, label %ge
lt:
  ret i16 %a
ge:
  ret i16 %b
}

; A compare with zero is a test.
; CHECK-LABEL: zero_test:
; CHECK:      test ax, ax
; CHECK-NEXT: je
define i16 @zero_test(i16 %a, i16 %b) {
  %c = icmp eq i16 %a, 0
  br i1 %c, label %z, label %nz
z:
  ret i16 %b
nz:
  ret i16 %a
}

; Signed and unsigned comparisons use different conditions.
; CHECK-LABEL: signed_unsigned:
; CHECK:      cmp {{[a-d]x}}, dx
; CHECK-NEXT: jge
; CHECK:      cmp {{[a-d]x}}, dx
; CHECK-NEXT: jb
define i16 @signed_unsigned(i16 %a, i16 %b) {
  %s = icmp slt i16 %a, %b
  br i1 %s, label %x, label %y
x:
  ret i16 1
y:
  %u = icmp ult i16 %a, %b
  %r = select i1 %u, i16 2, i16 3
  ret i16 %r
}

; A select is a branch over a move.
; CHECK-LABEL: umin:
; CHECK:      cmp ax, dx
; CHECK-NEXT: jb [[DONE:.LBB[0-9_]+]]
; CHECK:      mov ax, dx
; CHECK:      [[DONE]]:
define i16 @umin(i16 %a, i16 %b) {
  %c = icmp ult i16 %a, %b
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}

; A boolean value is built with a branch too (there is no setcc).
; CHECK-LABEL: bool:
; CHECK:      mov al, 1
; CHECK:      cmp
; CHECK-NEXT: je
; CHECK:      mov al, 0
define zeroext i1 @bool(i16 %a, i16 %b) {
  %c = icmp eq i16 %a, %b
  ret i1 %c
}

; A counted loop: increment, compare, backward branch.
; CHECK-LABEL: count:
; CHECK:      inc
; CHECK:      cmp
; CHECK-NEXT: jb
define i16 @count(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s1, %loop ]
  %i1 = add i16 %i, 1
  %s1 = add i16 %s, %i1
  %done = icmp uge i16 %i1, %n
  br i1 %done, label %exit, label %loop
exit:
  ret i16 %s1
}
