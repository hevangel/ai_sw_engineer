; 32-bit arithmetic, split into 16-bit pieces by the legalizer (see arith.ll).
; An i32 argument is two registers or stack slots: low word first.

target triple = "i8086"

define i32 @add32(i32 %a, i32 %b) {
  %r = add i32 %a, %b
  ret i32 %r
}

define i32 @sub32(i32 %a, i32 %b) {
  %r = sub i32 %a, %b
  ret i32 %r
}

define i32 @and32(i32 %a, i32 %b) {
  %r = and i32 %a, %b
  ret i32 %r
}

define i32 @or32(i32 %a, i32 %b) {
  %r = or i32 %a, %b
  ret i32 %r
}

define i32 @xor32(i32 %a, i32 %b) {
  %r = xor i32 %a, %b
  ret i32 %r
}

define i32 @neg32(i32 %a) {
  %r = sub i32 0, %a
  ret i32 %r
}

define i32 @shl32_16(i32 %a) {
  %r = shl i32 %a, 16
  ret i32 %r
}

define i32 @lshr32_16(i32 %a) {
  %r = lshr i32 %a, 16
  ret i32 %r
}

define i32 @shl32_8(i32 %a) {
  %r = shl i32 %a, 8
  ret i32 %r
}

define i32 @lshr32_8(i32 %a) {
  %r = lshr i32 %a, 8
  ret i32 %r
}

define i32 @ashr32_24(i32 %a) {
  %r = ashr i32 %a, 24
  ret i32 %r
}

define i16 @ult32(i32 %a, i32 %b) {
  %c = icmp ult i32 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @slt32(i32 %a, i32 %b) {
  %c = icmp slt i32 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @eq32(i32 %a, i32 %b) {
  %c = icmp eq i32 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i32 @umax32(i32 %a, i32 %b) {
  %c = icmp ugt i32 %a, %b
  %r = select i1 %c, i32 %a, i32 %b
  ret i32 %r
}

define i32 @add_imm32(i32 %a) {
  %r = add i32 %a, 100000
  ret i32 %r
}

; a 32-bit sum over a loop
define i32 @sum32(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %w = zext i16 %i to i32
  %m = shl i32 %w, 12
  %s1 = add i32 %s, %m
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, %n
  br i1 %more, label %loop, label %exit
exit:
  ret i32 %s1
}
