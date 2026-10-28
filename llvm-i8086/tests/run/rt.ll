; Calls into the runtime library (rt/): 32- and 64-bit divide, remainder,
; variable shifts, the 64-bit product, and the mem* functions (see arith.ll).

target triple = "i8086"

declare i32 @__mulsi3(i32, i32)
declare ptr @memcpy(ptr, ptr, i16)
declare ptr @memmove(ptr, ptr, i16)
declare ptr @memset(ptr, i16, i16)
declare i16 @memcmp(ptr, ptr, i16)
declare i16 @bcmp(ptr, ptr, i16)

define i32 @udiv32(i32 %a, i32 %b) {
  %r = udiv i32 %a, %b
  ret i32 %r
}

define i32 @urem32(i32 %a, i32 %b) {
  %r = urem i32 %a, %b
  ret i32 %r
}

define i32 @sdiv32(i32 %a, i32 %b) {
  %r = sdiv i32 %a, %b
  ret i32 %r
}

define i32 @srem32(i32 %a, i32 %b) {
  %r = srem i32 %a, %b
  ret i32 %r
}

define i32 @shl32v(i32 %a, i16 %n) {
  %c = zext i16 %n to i32
  %r = shl i32 %a, %c
  ret i32 %r
}

define i32 @lshr32v(i32 %a, i16 %n) {
  %c = zext i16 %n to i32
  %r = lshr i32 %a, %c
  ret i32 %r
}

define i32 @ashr32v(i32 %a, i16 %n) {
  %c = zext i16 %n to i32
  %r = ashr i32 %a, %c
  ret i32 %r
}

define i32 @mulsi3(i32 %a, i32 %b) {
  %r = call i32 @__mulsi3(i32 %a, i32 %b)
  ret i32 %r
}

define i64 @mul64(i64 %a, i64 %b) {
  %r = mul i64 %a, %b
  ret i64 %r
}

define i64 @udiv64(i64 %a, i64 %b) {
  %r = udiv i64 %a, %b
  ret i64 %r
}

define i64 @urem64(i64 %a, i64 %b) {
  %r = urem i64 %a, %b
  ret i64 %r
}

define i64 @sdiv64(i64 %a, i64 %b) {
  %r = sdiv i64 %a, %b
  ret i64 %r
}

define i64 @srem64(i64 %a, i64 %b) {
  %r = srem i64 %a, %b
  ret i64 %r
}

define i64 @shl64v(i64 %a, i16 %n) {
  %c = zext i16 %n to i64
  %r = shl i64 %a, %c
  ret i64 %r
}

define i64 @lshr64v(i64 %a, i16 %n) {
  %c = zext i16 %n to i64
  %r = lshr i64 %a, %c
  ret i64 %r
}

define i64 @ashr64v(i64 %a, i16 %n) {
  %c = zext i16 %n to i64
  %r = ashr i64 %a, %c
  ret i64 %r
}

; the mem* functions called by name, through pointers the test sets up
define ptr @call_memcpy(ptr %d, ptr %s, i16 %n) {
  %r = call ptr @memcpy(ptr %d, ptr %s, i16 %n)
  ret ptr %r
}

define ptr @call_memmove(ptr %d, ptr %s, i16 %n) {
  %r = call ptr @memmove(ptr %d, ptr %s, i16 %n)
  ret ptr %r
}

define ptr @call_memset(ptr %d, i16 %c, i16 %n) {
  %r = call ptr @memset(ptr %d, i16 %c, i16 %n)
  ret ptr %r
}

define i16 @call_memcmp(ptr %a, ptr %b, i16 %n) {
  %r = call i16 @memcmp(ptr %a, ptr %b, i16 %n)
  ret i16 %r
}

define i16 @call_bcmp(ptr %a, ptr %b, i16 %n) {
  %r = call i16 @bcmp(ptr %a, ptr %b, i16 %n)
  ret i16 %r
}
