; Extensions and truncations between i8, i16 and i32 (see arith.ll).

target triple = "i8086"

define i16 @zext8(i8 %a) {
  %r = zext i8 %a to i16
  ret i16 %r
}

define i16 @sext8(i8 %a) {
  %r = sext i8 %a to i16
  ret i16 %r
}

define zeroext i8 @trunc16(i16 %a) {
  %r = trunc i16 %a to i8
  ret i8 %r
}

define zeroext i8 @trunc_hi(i16 %a) {
  %s = lshr i16 %a, 8
  %r = trunc i16 %s to i8
  ret i8 %r
}

define i16 @sext_inreg(i16 %a) {
  %t = trunc i16 %a to i8
  %r = sext i8 %t to i16
  ret i16 %r
}

; i16 -> i32 in DX:AX
define i32 @zext32(i16 %a) {
  %r = zext i16 %a to i32
  ret i32 %r
}

define i32 @sext32(i16 %a) {
  %r = sext i16 %a to i32
  ret i32 %r
}

define i16 @trunc32(i32 %a) {
  %r = trunc i32 %a to i16
  ret i16 %r
}

define i16 @hi16(i32 %a) {
  %s = lshr i32 %a, 16
  %r = trunc i32 %s to i16
  ret i16 %r
}

; a byte add that must not leak into the neighbouring byte
define i16 @add_bytes(i16 %a, i16 %b) {
  %x = trunc i16 %a to i8
  %y = trunc i16 %b to i8
  %s = add i8 %x, %y
  %r = zext i8 %s to i16
  ret i16 %r
}
