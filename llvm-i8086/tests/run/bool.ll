; Comparisons as values (the borrow-flag sequences of LowerSETCC). See arith.ll for how this file is used.

target triple = "i8086"

define i16 @b16_eq(i16 %a, i16 %b) {
  %c = icmp eq i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_eq(i8 %a, i8 %b) {
  %c = icmp eq i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_eq(i16 %a) {
  %c = icmp eq i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_ne(i16 %a, i16 %b) {
  %c = icmp ne i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_ne(i8 %a, i8 %b) {
  %c = icmp ne i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_ne(i16 %a) {
  %c = icmp ne i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_ult(i16 %a, i16 %b) {
  %c = icmp ult i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_ult(i8 %a, i8 %b) {
  %c = icmp ult i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_ult(i16 %a) {
  %c = icmp ult i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_ule(i16 %a, i16 %b) {
  %c = icmp ule i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_ule(i8 %a, i8 %b) {
  %c = icmp ule i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_ule(i16 %a) {
  %c = icmp ule i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_ugt(i16 %a, i16 %b) {
  %c = icmp ugt i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_ugt(i8 %a, i8 %b) {
  %c = icmp ugt i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_ugt(i16 %a) {
  %c = icmp ugt i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_uge(i16 %a, i16 %b) {
  %c = icmp uge i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_uge(i8 %a, i8 %b) {
  %c = icmp uge i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_uge(i16 %a) {
  %c = icmp uge i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_slt(i16 %a, i16 %b) {
  %c = icmp slt i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_slt(i8 %a, i8 %b) {
  %c = icmp slt i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_slt(i16 %a) {
  %c = icmp slt i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_sle(i16 %a, i16 %b) {
  %c = icmp sle i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_sle(i8 %a, i8 %b) {
  %c = icmp sle i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_sle(i16 %a) {
  %c = icmp sle i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_sgt(i16 %a, i16 %b) {
  %c = icmp sgt i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_sgt(i8 %a, i8 %b) {
  %c = icmp sgt i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_sgt(i16 %a) {
  %c = icmp sgt i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b16_sge(i16 %a, i16 %b) {
  %c = icmp sge i16 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @b8_sge(i8 %a, i8 %b) {
  %c = icmp sge i8 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @bk_sge(i16 %a) {
  %c = icmp sge i16 %a, 1000
  %r = zext i1 %c to i16
  ret i16 %r
}

; the value of a comparison used in arithmetic and as a stored byte
define i16 @count_lt(i16 %a, i16 %b, i16 %c) {
  %x = icmp slt i16 %a, %b
  %y = icmp ult i16 %b, %c
  %z = icmp ne i16 %a, %c
  %xs = zext i1 %x to i16
  %ys = zext i1 %y to i16
  %zs = zext i1 %z to i16
  %s1 = add i16 %xs, %ys
  %s2 = add i16 %s1, %zs
  ret i16 %s2
}
