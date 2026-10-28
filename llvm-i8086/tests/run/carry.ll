; Add and subtract with carry: 64-bit arithmetic (arguments on the stack, the
; result through a hidden pointer) and the overflow intrinsics (see arith.ll).

target triple = "i8086"

declare { i16, i1 } @llvm.uadd.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.usub.with.overflow.i16(i16, i16)

define i64 @add64(i64 %a, i64 %b) {
  %r = add i64 %a, %b
  ret i64 %r
}

define i64 @sub64(i64 %a, i64 %b) {
  %r = sub i64 %a, %b
  ret i64 %r
}

define i64 @neg64(i64 %a) {
  %r = sub i64 0, %a
  ret i64 %r
}

define i64 @and64(i64 %a, i64 %b) {
  %r = and i64 %a, %b
  ret i64 %r
}

define i64 @xor64(i64 %a, i64 %b) {
  %r = xor i64 %a, %b
  ret i64 %r
}

define i64 @shl64_16(i64 %a) {
  %r = shl i64 %a, 16
  ret i64 %r
}

define i64 @lshr64_48(i64 %a) {
  %r = lshr i64 %a, 48
  ret i64 %r
}

define i64 @add_imm64(i64 %a) {
  %r = add i64 %a, 4294967297
  ret i64 %r
}

define i16 @ult64(i64 %a, i64 %b) {
  %c = icmp ult i64 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @slt64(i64 %a, i64 %b) {
  %c = icmp slt i64 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @eq64(i64 %a, i64 %b) {
  %c = icmp eq i64 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @uadd_carry(i16 %a, i16 %b) {
  %p = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %b)
  %c = extractvalue { i16, i1 } %p, 1
  %r = zext i1 %c to i16
  ret i16 %r
}

define i16 @usub_borrow(i16 %a, i16 %b) {
  %p = call { i16, i1 } @llvm.usub.with.overflow.i16(i16 %a, i16 %b)
  %c = extractvalue { i16, i1 } %p, 1
  %r = zext i1 %c to i16
  ret i16 %r
}

; the sum and the carry both used
define i16 @sat_add(i16 %a, i16 %b) {
  %p = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %b)
  %s = extractvalue { i16, i1 } %p, 0
  %c = extractvalue { i16, i1 } %p, 1
  %r = select i1 %c, i16 -1, i16 %s
  ret i16 %r
}
