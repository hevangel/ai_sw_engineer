; Comparisons, selects and branches (see arith.ll for how this file is used).

target triple = "i8086"

define i16 @eq(i16 %a, i16 %b) {
  %c = icmp eq i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @ne(i16 %a, i16 %b) {
  %c = icmp ne i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @ult(i16 %a, i16 %b) {
  %c = icmp ult i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @ule(i16 %a, i16 %b) {
  %c = icmp ule i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @ugt(i16 %a, i16 %b) {
  %c = icmp ugt i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @uge(i16 %a, i16 %b) {
  %c = icmp uge i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @slt(i16 %a, i16 %b) {
  %c = icmp slt i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @sle(i16 %a, i16 %b) {
  %c = icmp sle i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @sgt(i16 %a, i16 %b) {
  %c = icmp sgt i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @sge(i16 %a, i16 %b) {
  %c = icmp sge i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @is_zero(i16 %a) {
  %c = icmp eq i16 %a, 0
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @slt_imm(i16 %a) {
  %c = icmp slt i16 %a, -100
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @ugt_imm(i16 %a) {
  %c = icmp ugt i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define zeroext i8 @ult8(i8 zeroext %a, i8 zeroext %b) {
  %c = icmp ult i8 %a, %b
  %r = zext i1 %c to i8
  ret i8 %r
}

define zeroext i8 @slt8(i8 zeroext %a, i8 zeroext %b) {
  %c = icmp slt i8 %a, %b
  %r = zext i1 %c to i8
  ret i8 %r
}

define i16 @umin(i16 %a, i16 %b) {
  %c = icmp ult i16 %a, %b
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}

define i16 @smax(i16 %a, i16 %b) {
  %c = icmp sgt i16 %a, %b
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}

define i16 @abs16(i16 %a) {
  %neg = icmp slt i16 %a, 0
  %n = sub i16 0, %a
  %r = select i1 %neg, i16 %n, i16 %a
  ret i16 %r
}

define zeroext i8 @umax8(i8 zeroext %a, i8 zeroext %b) {
  %c = icmp ugt i8 %a, %b
  %r = select i1 %c, i8 %a, i8 %b
  ret i8 %r
}

; if / else with different work in each arm
define i16 @clamp(i16 %a, i16 %lo, i16 %hi) {
entry:
  %below = icmp slt i16 %a, %lo
  br i1 %below, label %low, label %check
low:
  ret i16 %lo
check:
  %above = icmp sgt i16 %a, %hi
  br i1 %above, label %high, label %mid
high:
  ret i16 %hi
mid:
  ret i16 %a
}

; 1 + 2 + ... + n (n unsigned, a counted loop)
define i16 @sum_to(i16 %n) {
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

; Greatest common divisor by subtraction (no divide needed)
define i16 @gcd(i16 %a, i16 %b) {
entry:
  %az = icmp eq i16 %a, 0
  br i1 %az, label %ret_b, label %loop
loop:
  %x = phi i16 [ %a, %entry ], [ %x2, %next ]
  %y = phi i16 [ %b, %entry ], [ %y2, %next ]
  %yz = icmp eq i16 %y, 0
  br i1 %yz, label %ret_x, label %next
next:
  %lt = icmp ult i16 %x, %y
  %big = select i1 %lt, i16 %y, i16 %x
  %small = select i1 %lt, i16 %x, i16 %y
  %x2 = sub i16 %big, %small
  %y2 = select i1 %lt, i16 %x, i16 %y
  br label %loop
ret_x:
  ret i16 %x
ret_b:
  ret i16 %b
}
