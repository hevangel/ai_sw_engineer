; Far code: functions with the far calling convention (cc 1000) return with retf
; and are called with the segment half of the return address pushed first; a call
; through a far pointer reads an offset:segment slot (spec/abi.md D7).

target triple = "i8086"

@counter = global i16 0

; five arguments: two in registers' worth and the rest on the stack, which starts
; four bytes above the frame pointer in a far function
define cc 1000 i16 @far_sum5(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e) {
  %s1 = add i16 %a, %b
  %s2 = add i16 %s1, %c
  %s3 = add i16 %s2, %d
  %s4 = add i16 %s3, %e
  %s5 = mul i16 %s4, 3
  ret i16 %s5
}

; keeps a value in a callee-saved register across a far call
define cc 1000 i16 @far_bump(i16 %x) {
  %c = load i16, ptr @counter
  %n = add i16 %c, %x
  store i16 %n, ptr @counter
  ret i16 %n
}

define cc 1000 i32 @far_wide(i32 %x) {
  %y = add i32 %x, 100000
  ret i32 %y
}

define i16 @call_direct(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e) {
  %r = call cc 1000 i16 @far_sum5(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e)
  %r2 = add i16 %r, 1
  ret i16 %r2
}

; the callee through a far pointer: the segment is CS (= DS here)
define i16 @call_indirect(ptr addrspace(1) %f, i16 %a, i16 %b, i16 %c, i16 %d, i16 %e) {
  %r = call cc 1000 addrspace(1) i16 %f(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e)
  %r2 = add i16 %r, 1
  ret i16 %r2
}

define ptr addrspace(1) @far_sum5_address() {
  ret ptr addrspace(1) addrspacecast (ptr @far_sum5 to ptr addrspace(1))
}

define i32 @call_wide(i32 %x) {
  %r = call cc 1000 i32 @far_wide(i32 %x)
  ret i32 %r
}

; a loop of far calls, the loop state living across them
define i16 @far_loop(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %acc = phi i16 [ 0, %entry ], [ %acc1, %loop ]
  %acc1 = call cc 1000 i16 @far_bump(i16 %i)
  %i1 = add i16 %i, 1
  %done = icmp eq i16 %i1, %n
  br i1 %done, label %out, label %loop
out:
  ret i16 %acc1
}
