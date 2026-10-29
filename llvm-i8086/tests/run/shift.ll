; Shifts by constants and by a variable count (see arith.ll).

target triple = "i8086"

define i16 @shl1(i16 %a) {
  %r = shl i16 %a, 1
  ret i16 %r
}

define i16 @shl2(i16 %a) {
  %r = shl i16 %a, 2
  ret i16 %r
}

define i16 @shl5(i16 %a) {
  %r = shl i16 %a, 5
  ret i16 %r
}

define i16 @shl8(i16 %a) {
  %r = shl i16 %a, 8
  ret i16 %r
}

define i16 @lshr1(i16 %a) {
  %r = lshr i16 %a, 1
  ret i16 %r
}

define i16 @lshr3(i16 %a) {
  %r = lshr i16 %a, 3
  ret i16 %r
}

define i16 @lshr9(i16 %a) {
  %r = lshr i16 %a, 9
  ret i16 %r
}

define i16 @ashr1(i16 %a) {
  %r = ashr i16 %a, 1
  ret i16 %r
}

define i16 @ashr4(i16 %a) {
  %r = ashr i16 %a, 4
  ret i16 %r
}

define i16 @ashr15(i16 %a) {
  %r = ashr i16 %a, 15
  ret i16 %r
}

define i16 @shl_var(i16 %a, i16 %n) {
  %m = and i16 %n, 15
  %r = shl i16 %a, %m
  ret i16 %r
}

define i16 @lshr_var(i16 %a, i16 %n) {
  %m = and i16 %n, 15
  %r = lshr i16 %a, %m
  ret i16 %r
}

define i16 @ashr_var(i16 %a, i16 %n) {
  %m = and i16 %n, 15
  %r = ashr i16 %a, %m
  ret i16 %r
}

define zeroext i8 @shl8_const(i8 zeroext %a) {
  %r = shl i8 %a, 3
  ret i8 %r
}

define zeroext i8 @lshr8_const(i8 zeroext %a) {
  %r = lshr i8 %a, 2
  ret i8 %r
}

define zeroext i8 @ashr8_const(i8 zeroext %a) {
  %r = ashr i8 %a, 3
  ret i8 %r
}

define zeroext i8 @shl8_var(i8 zeroext %a, i8 zeroext %n) {
  %m = and i8 %n, 7
  %r = shl i8 %a, %m
  ret i8 %r
}
