; Native multiply and divide (see arith.ll for how this file is used).

target triple = "i8086"

define i16 @mul16(i16 %a, i16 %b) {
  %r = mul i16 %a, %b
  ret i16 %r
}

define i16 @mul10(i16 %a) {
  %r = mul i16 %a, 10
  ret i16 %r
}

define i16 @mul_big(i16 %a) {
  %r = mul i16 %a, 4661
  ret i16 %r
}

; the high half of an unsigned / signed 16x16 product
define i16 @umulhi(i16 %a, i16 %b) {
  %x = zext i16 %a to i32
  %y = zext i16 %b to i32
  %p = mul i32 %x, %y
  %s = lshr i32 %p, 16
  %r = trunc i32 %s to i16
  ret i16 %r
}

define i16 @smulhi(i16 %a, i16 %b) {
  %x = sext i16 %a to i32
  %y = sext i16 %b to i32
  %p = mul i32 %x, %y
  %s = lshr i32 %p, 16
  %r = trunc i32 %s to i16
  ret i16 %r
}

define i32 @mul32(i32 %a, i32 %b) {
  %r = mul i32 %a, %b
  ret i32 %r
}

define i16 @udiv16(i16 %a, i16 %b) {
  %r = udiv i16 %a, %b
  ret i16 %r
}

define i16 @urem16(i16 %a, i16 %b) {
  %r = urem i16 %a, %b
  ret i16 %r
}

define i16 @sdiv16(i16 %a, i16 %b) {
  %r = sdiv i16 %a, %b
  ret i16 %r
}

define i16 @srem16(i16 %a, i16 %b) {
  %r = srem i16 %a, %b
  ret i16 %r
}

; quotient and remainder of one division
define i16 @divmod(i16 %a, i16 %b) {
  %q = udiv i16 %a, %b
  %m = urem i16 %a, %b
  %t = mul i16 %m, 3
  %r = add i16 %q, %t
  ret i16 %r
}

define zeroext i8 @udiv8(i8 zeroext %a, i8 zeroext %b) {
  %r = udiv i8 %a, %b
  ret i8 %r
}

define zeroext i8 @sdiv8(i8 zeroext %a, i8 zeroext %b) {
  %r = sdiv i8 %a, %b
  ret i8 %r
}

define zeroext i8 @mul8(i8 zeroext %a, i8 zeroext %b) {
  %r = mul i8 %a, %b
  ret i8 %r
}

define i16 @udiv10(i16 %a) {
  %r = udiv i16 %a, 10
  ret i16 %r
}

define i16 @urem10(i16 %a) {
  %r = urem i16 %a, 10
  ret i16 %r
}

define i16 @sdiv3(i16 %a) {
  %r = sdiv i16 %a, 3
  ret i16 %r
}

define i16 @sdivm7(i16 %a) {
  %r = sdiv i16 %a, -7
  ret i16 %r
}

define i16 @udiv8c(i16 %a) {
  %r = udiv i16 %a, 8
  ret i16 %r
}

define i16 @srem16c(i16 %a) {
  %r = srem i16 %a, 16
  ret i16 %r
}
