; 64-bit multiply, divide, remainder and shifts for llvm-i8086.
;
; Written in LLVM IR so the calling convention (spec/abi.md D3: a 64-bit value
; is four words, the result goes through a hidden pointer in AX) is the
; compiler's own. Nothing here uses an operation that is itself a call to these
; helpers: only constant shifts, add, subtract, compare and 16x16 products.
; Division by zero returns all ones for the quotient and the dividend for the
; remainder (the restoring division below never traps).

target triple = "i8086"

; --- multiply: the low 64 bits, from 16x16 -> 32 partial products ----------

define i64 @__muldi3(i64 %a, i64 %b) {
  %a1s = lshr i64 %a, 16
  %a2s = lshr i64 %a, 32
  %a3s = lshr i64 %a, 48
  %b1s = lshr i64 %b, 16
  %b2s = lshr i64 %b, 32
  %b3s = lshr i64 %b, 48
  %a0 = trunc i64 %a to i16
  %a1 = trunc i64 %a1s to i16
  %a2 = trunc i64 %a2s to i16
  %a3 = trunc i64 %a3s to i16
  %b0 = trunc i64 %b to i16
  %b1 = trunc i64 %b1s to i16
  %b2 = trunc i64 %b2s to i16
  %b3 = trunc i64 %b3s to i16
  %za0 = zext i16 %a0 to i32
  %za1 = zext i16 %a1 to i32
  %za2 = zext i16 %a2 to i32
  %za3 = zext i16 %a3 to i32
  %zb0 = zext i16 %b0 to i32
  %zb1 = zext i16 %b1 to i32
  %zb2 = zext i16 %b2 to i32
  %zb3 = zext i16 %b3 to i32

  ; weight 0
  %p00 = mul i32 %za0, %zb0
  %q00 = zext i32 %p00 to i64
  ; weight 16
  %p01 = mul i32 %za0, %zb1
  %p10 = mul i32 %za1, %zb0
  %q01 = zext i32 %p01 to i64
  %q10 = zext i32 %p10 to i64
  %s1 = add i64 %q01, %q10
  %w1 = shl i64 %s1, 16
  ; weight 32
  %p02 = mul i32 %za0, %zb2
  %p11 = mul i32 %za1, %zb1
  %p20 = mul i32 %za2, %zb0
  %q02 = zext i32 %p02 to i64
  %q11 = zext i32 %p11 to i64
  %q20 = zext i32 %p20 to i64
  %s2a = add i64 %q02, %q11
  %s2 = add i64 %s2a, %q20
  %w2 = shl i64 %s2, 32
  ; weight 48: only the low 16 bits of each product matter
  %p03 = mul i16 %a0, %b3
  %p12 = mul i16 %a1, %b2
  %p21 = mul i16 %a2, %b1
  %p30 = mul i16 %a3, %b0
  %s3a = add i16 %p03, %p12
  %s3b = add i16 %s3a, %p21
  %s3 = add i16 %s3b, %p30
  %q3 = zext i16 %s3 to i64
  %w3 = shl i64 %q3, 48

  %r0 = add i64 %q00, %w1
  %r1 = add i64 %r0, %w2
  %r2 = add i64 %r1, %w3
  ret i64 %r2
}

; --- shifts: by the bits of the count, each a constant shift ---------------

define i64 @__ashldi3(i64 %a, i8 %n) {
  %c = zext i8 %n to i16
  %b0 = and i16 %c, 1
  %t0 = icmp ne i16 %b0, 0
  %s0 = shl i64 %a, 1
  %x0 = select i1 %t0, i64 %s0, i64 %a
  %b1 = and i16 %c, 2
  %t1 = icmp ne i16 %b1, 0
  %s1 = shl i64 %x0, 2
  %x1 = select i1 %t1, i64 %s1, i64 %x0
  %b2 = and i16 %c, 4
  %t2 = icmp ne i16 %b2, 0
  %s2 = shl i64 %x1, 4
  %x2 = select i1 %t2, i64 %s2, i64 %x1
  %b3 = and i16 %c, 8
  %t3 = icmp ne i16 %b3, 0
  %s3 = shl i64 %x2, 8
  %x3 = select i1 %t3, i64 %s3, i64 %x2
  %b4 = and i16 %c, 16
  %t4 = icmp ne i16 %b4, 0
  %s4 = shl i64 %x3, 16
  %x4 = select i1 %t4, i64 %s4, i64 %x3
  %b5 = and i16 %c, 32
  %t5 = icmp ne i16 %b5, 0
  %s5 = shl i64 %x4, 32
  %x5 = select i1 %t5, i64 %s5, i64 %x4
  ret i64 %x5
}

define i64 @__lshrdi3(i64 %a, i8 %n) {
  %c = zext i8 %n to i16
  %b0 = and i16 %c, 1
  %t0 = icmp ne i16 %b0, 0
  %s0 = lshr i64 %a, 1
  %x0 = select i1 %t0, i64 %s0, i64 %a
  %b1 = and i16 %c, 2
  %t1 = icmp ne i16 %b1, 0
  %s1 = lshr i64 %x0, 2
  %x1 = select i1 %t1, i64 %s1, i64 %x0
  %b2 = and i16 %c, 4
  %t2 = icmp ne i16 %b2, 0
  %s2 = lshr i64 %x1, 4
  %x2 = select i1 %t2, i64 %s2, i64 %x1
  %b3 = and i16 %c, 8
  %t3 = icmp ne i16 %b3, 0
  %s3 = lshr i64 %x2, 8
  %x3 = select i1 %t3, i64 %s3, i64 %x2
  %b4 = and i16 %c, 16
  %t4 = icmp ne i16 %b4, 0
  %s4 = lshr i64 %x3, 16
  %x4 = select i1 %t4, i64 %s4, i64 %x3
  %b5 = and i16 %c, 32
  %t5 = icmp ne i16 %b5, 0
  %s5 = lshr i64 %x4, 32
  %x5 = select i1 %t5, i64 %s5, i64 %x4
  ret i64 %x5
}

define i64 @__ashrdi3(i64 %a, i8 %n) {
  %c = zext i8 %n to i16
  %b0 = and i16 %c, 1
  %t0 = icmp ne i16 %b0, 0
  %s0 = ashr i64 %a, 1
  %x0 = select i1 %t0, i64 %s0, i64 %a
  %b1 = and i16 %c, 2
  %t1 = icmp ne i16 %b1, 0
  %s1 = ashr i64 %x0, 2
  %x1 = select i1 %t1, i64 %s1, i64 %x0
  %b2 = and i16 %c, 4
  %t2 = icmp ne i16 %b2, 0
  %s2 = ashr i64 %x1, 4
  %x2 = select i1 %t2, i64 %s2, i64 %x1
  %b3 = and i16 %c, 8
  %t3 = icmp ne i16 %b3, 0
  %s3 = ashr i64 %x2, 8
  %x3 = select i1 %t3, i64 %s3, i64 %x2
  %b4 = and i16 %c, 16
  %t4 = icmp ne i16 %b4, 0
  %s4 = ashr i64 %x3, 16
  %x4 = select i1 %t4, i64 %s4, i64 %x3
  %b5 = and i16 %c, 32
  %t5 = icmp ne i16 %b5, 0
  %s5 = ashr i64 %x4, 32
  %x5 = select i1 %t5, i64 %s5, i64 %x4
  ret i64 %x5
}

; --- divide: restoring division, one quotient bit per step -----------------
;
; r is the partial remainder, n is shifted left so its top bit feeds r. When
; r*2+bit overflows 64 bits (only possible if d >= 2^63) the true value is
; certainly >= d, which %ovf records.

define internal i64 @udivmod64(i64 %n, i64 %d, ptr %rem) {
entry:
  br label %loop

loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %nn = phi i64 [ %n, %entry ], [ %nn1, %loop ]
  %r = phi i64 [ 0, %entry ], [ %r2, %loop ]
  %q = phi i64 [ 0, %entry ], [ %q1, %loop ]
  %top = lshr i64 %nn, 63
  %nn1 = shl i64 %nn, 1
  %ovf = lshr i64 %r, 63
  %rs = shl i64 %r, 1
  %r1 = or i64 %rs, %top
  %ovfb = icmp ne i64 %ovf, 0
  %big = icmp uge i64 %r1, %d
  %ge = or i1 %ovfb, %big
  %rsub = sub i64 %r1, %d
  %r2 = select i1 %ge, i64 %rsub, i64 %r1
  %qs = shl i64 %q, 1
  %bit = zext i1 %ge to i64
  %q1 = or i64 %qs, %bit
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, 64
  br i1 %more, label %loop, label %done

done:
  store i64 %r2, ptr %rem
  ret i64 %q1
}

define i64 @__udivdi3(i64 %n, i64 %d) {
  %rem = alloca i64, align 2
  %q = call i64 @udivmod64(i64 %n, i64 %d, ptr %rem)
  ret i64 %q
}

define i64 @__umoddi3(i64 %n, i64 %d) {
  %rem = alloca i64, align 2
  %q = call i64 @udivmod64(i64 %n, i64 %d, ptr %rem)
  %r = load i64, ptr %rem, align 2
  ret i64 %r
}

; signed: divide the magnitudes, the quotient is negative when the signs
; differ, the remainder takes the dividend's sign.

define i64 @__divdi3(i64 %n, i64 %d) {
  %rem = alloca i64, align 2
  %nneg = icmp slt i64 %n, 0
  %dneg = icmp slt i64 %d, 0
  %nn = sub i64 0, %n
  %dn = sub i64 0, %d
  %na = select i1 %nneg, i64 %nn, i64 %n
  %da = select i1 %dneg, i64 %dn, i64 %d
  %q = call i64 @udivmod64(i64 %na, i64 %da, ptr %rem)
  %neg = xor i1 %nneg, %dneg
  %qn = sub i64 0, %q
  %r = select i1 %neg, i64 %qn, i64 %q
  ret i64 %r
}

define i64 @__moddi3(i64 %n, i64 %d) {
  %rem = alloca i64, align 2
  %nneg = icmp slt i64 %n, 0
  %dneg = icmp slt i64 %d, 0
  %nn = sub i64 0, %n
  %dn = sub i64 0, %d
  %na = select i1 %nneg, i64 %nn, i64 %n
  %da = select i1 %dneg, i64 %dn, i64 %d
  %q = call i64 @udivmod64(i64 %na, i64 %da, ptr %rem)
  %m = load i64, ptr %rem, align 2
  %mn = sub i64 0, %m
  %r = select i1 %nneg, i64 %mn, i64 %m
  ret i64 %r
}
