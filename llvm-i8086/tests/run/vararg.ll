; Variadic functions and byval arguments (spec/abi.md D3a). See arith.ll for how
; this file is used: it is compiled to vararg.o, linked, and called from
; system/ibm_pc/emulator/tests/i8086_codegen.rs.

target triple = "i8086"

declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
declare void @llvm.va_copy.p0(ptr, ptr)

; The sum of the n 16-bit variadic arguments.
define i16 @sum16(i16 %n, ...) {
entry:
  %ap = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %body ]
  %acc = phi i16 [ 0, %entry ], [ %acc2, %body ]
  %done = icmp eq i16 %i, %n
  br i1 %done, label %out, label %body
body:
  %v = va_arg ptr %ap, i16
  %acc2 = add i16 %acc, %v
  %i2 = add i16 %i, 1
  br label %loop
out:
  call void @llvm.va_end.p0(ptr %ap)
  ret i16 %acc
}

; The sum of the n 32-bit variadic arguments.
define i32 @sum32(i16 %n, ...) {
entry:
  %ap = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %body ]
  %acc = phi i32 [ 0, %entry ], [ %acc2, %body ]
  %done = icmp eq i16 %i, %n
  br i1 %done, label %out, label %body
body:
  %v = va_arg ptr %ap, i32
  %acc2 = add i32 %acc, %v
  %i2 = add i16 %i, 1
  br label %loop
out:
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %acc
}

; A byte, a 32-bit and a 64-bit variadic argument, folded into 16 bits.
define i16 @mix(i16 %tag, ...) {
  %ap = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  %c = va_arg ptr %ap, i8
  %w = va_arg ptr %ap, i32
  %q = va_arg ptr %ap, i64
  call void @llvm.va_end.p0(ptr %ap)
  %c16 = zext i8 %c to i16
  %w0 = trunc i32 %w to i16
  %w1s = lshr i32 %w, 16
  %w1 = trunc i32 %w1s to i16
  %q0 = trunc i64 %q to i16
  %q3s = lshr i64 %q, 48
  %q3 = trunc i64 %q3s to i16
  %r0 = add i16 %tag, %c16
  %r1 = add i16 %r0, %w0
  %r2 = xor i16 %r1, %w1
  %r3 = add i16 %r2, %q0
  %r4 = xor i16 %r3, %q3
  ret i16 %r4
}

; va_copy: the arguments are read twice, once through each list.
define i16 @sum_twice(i16 %n, ...) {
entry:
  %ap = alloca ptr
  %bp = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  call void @llvm.va_copy.p0(ptr %bp, ptr %ap)
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %body ]
  %acc = phi i16 [ 0, %entry ], [ %acc3, %body ]
  %done = icmp eq i16 %i, %n
  br i1 %done, label %out, label %body
body:
  %v = va_arg ptr %ap, i16
  %u = va_arg ptr %bp, i16
  %acc2 = add i16 %acc, %v
  %acc3 = add i16 %acc2, %u
  %i2 = add i16 %i, 1
  br label %loop
out:
  call void @llvm.va_end.p0(ptr %ap)
  call void @llvm.va_end.p0(ptr %bp)
  ret i16 %acc
}

; Callers: every argument of a variadic call, fixed or not, is on the stack.
define i16 @call_sum16(i16 %a, i16 %b, i16 %c, i16 %d) {
  %r = call i16 (i16, ...) @sum16(i16 4, i16 %a, i16 %b, i16 %c, i16 %d)
  ret i16 %r
}

define i32 @call_sum32(i16 %x, i16 %y) {
  %xs = zext i16 %x to i32
  %xh = shl i32 %xs, 16
  %a = or i32 %xh, 7
  %ys = zext i16 %y to i32
  %r = call i32 (i16, ...) @sum32(i16 3, i32 %a, i32 %ys, i32 305419896)
  ret i32 %r
}

define i16 @call_mix(i16 %x, i16 %y) {
  %c = trunc i16 %x to i8
  %xs = zext i16 %x to i32
  %xh = shl i32 %xs, 16
  %ys = zext i16 %y to i32
  %w = or i32 %xh, %ys
  %q0 = zext i32 %w to i64
  %q1 = shl i64 %q0, 24
  %q = or i64 %q1, 255
  %r = call i16 (i16, ...) @mix(i16 9, i8 %c, i32 %w, i64 %q)
  ret i16 %r
}

define i16 @call_sum_twice(i16 %a, i16 %b) {
  %r = call i16 (i16, ...) @sum_twice(i16 2, i16 %a, i16 %b)
  ret i16 %r
}

; byval: a 5-byte packed object. The callee changes its copy; the caller's
; object must not change.
%P = type <{ i16, i16, i8 }>

define i16 @byval_odd(ptr byval(%P) align 2 %p) {
  %a = load i16, ptr %p
  %pb = getelementptr inbounds %P, ptr %p, i16 0, i32 1
  %b = load i16, ptr %pb
  %pc = getelementptr inbounds %P, ptr %p, i16 0, i32 2
  %c = load i8, ptr %pc
  %c16 = zext i8 %c to i16
  store i16 0, ptr %p
  %b2 = shl i16 %b, 1
  %c3 = mul i16 %c16, 3
  %s0 = add i16 %a, %b2
  %s1 = add i16 %s0, %c3
  ret i16 %s1
}

define i16 @call_byval_odd(i16 %x, i16 %y) {
  %s = alloca %P, align 2
  store i16 %x, ptr %s
  %sb = getelementptr inbounds %P, ptr %s, i16 0, i32 1
  store i16 %y, ptr %sb
  %sc = getelementptr inbounds %P, ptr %s, i16 0, i32 2
  %z = xor i16 %x, %y
  %zb = trunc i16 %z to i8
  store i8 %zb, ptr %sc
  %r = call i16 @byval_odd(ptr byval(%P) align 2 %s)
  %after = load i16, ptr %s
  %res = xor i16 %r, %after
  ret i16 %res
}

; byval between register arguments: a in AX, the object on the stack, b in DX.
%Q = type { i16, i16, i16, i16 }

define i16 @byval_mid(i16 %a, ptr byval(%Q) align 2 %q, i16 %b) {
  %p1 = getelementptr inbounds %Q, ptr %q, i16 0, i32 1
  %p3 = getelementptr inbounds %Q, ptr %q, i16 0, i32 3
  %v1 = load i16, ptr %p1
  %v3 = load i16, ptr %p3
  %s0 = sub i16 %a, %b
  %s1 = add i16 %s0, %v1
  %s2 = xor i16 %s1, %v3
  ret i16 %s2
}

define i16 @call_byval_mid(i16 %x, i16 %y) {
  %s = alloca %Q, align 2
  store %Q { i16 1, i16 2, i16 3, i16 4 }, ptr %s
  %p3 = getelementptr inbounds %Q, ptr %s, i16 0, i32 3
  store i16 %y, ptr %p3
  %r = call i16 @byval_mid(i16 %x, ptr byval(%Q) align 2 %s, i16 %y)
  ret i16 %r
}

; byval of a big object (200 bytes): room on the stack and a block copy.
%Big = type { [100 x i16] }

define i16 @byval_big(i16 %k, ptr byval(%Big) align 2 %b) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %acc = phi i16 [ 0, %entry ], [ %acc2, %loop ]
  %p = getelementptr inbounds %Big, ptr %b, i16 0, i32 0, i16 %i
  %v = load i16, ptr %p
  store i16 0, ptr %p
  %acc2 = add i16 %acc, %v
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 100
  br i1 %c, label %loop, label %out
out:
  %r = add i16 %acc2, %k
  ret i16 %r
}

define i16 @call_byval_big(i16 %x) {
entry:
  %s = alloca %Big, align 2
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %i2, %fill ]
  %p = getelementptr inbounds %Big, ptr %s, i16 0, i32 0, i16 %i
  %v = add i16 %x, %i
  store i16 %v, ptr %p
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 100
  br i1 %c, label %fill, label %call
call:
  %r = call i16 @byval_big(i16 7, ptr byval(%Big) align 2 %s)
  %p5 = getelementptr inbounds %Big, ptr %s, i16 0, i32 0, i16 5
  %after = load i16, ptr %p5
  %res = xor i16 %r, %after
  ret i16 %res
}
