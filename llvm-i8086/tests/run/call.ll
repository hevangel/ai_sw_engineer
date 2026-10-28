; Calls, stack arguments, recursion and callee-saved registers (see arith.ll).

target triple = "i8086"

define i16 @id(i16 %a) noinline {
  ret i16 %a
}

define i16 @add3(i16 %a, i16 %b, i16 %c) noinline {
  %s = add i16 %a, %b
  %r = add i16 %s, %c
  ret i16 %r
}

; arguments 4 and 5 are on the stack (abi.md D3)
define i16 @weigh5(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e) noinline {
  %b2 = shl i16 %b, 1
  %c3 = add i16 %c, %c
  %c3b = add i16 %c3, %c
  %d4 = shl i16 %d, 2
  %e5 = shl i16 %e, 3
  %s1 = add i16 %a, %b2
  %s2 = add i16 %s1, %c3b
  %s3 = add i16 %s2, %d4
  %r = add i16 %s3, %e5
  ret i16 %r
}

define i16 @call_weigh5(i16 %x, i16 %y) {
  %a = call i16 @weigh5(i16 %x, i16 %y, i16 %x, i16 %y, i16 %x)
  %b = call i16 @weigh5(i16 1, i16 2, i16 3, i16 4, i16 %a)
  ret i16 %b
}

; seven arguments: three in registers, four pushed
define i16 @sum7(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e, i16 %f, i16 %g) noinline {
  %s1 = sub i16 %a, %b
  %s2 = add i16 %s1, %c
  %s3 = sub i16 %s2, %d
  %s4 = add i16 %s3, %e
  %s5 = sub i16 %s4, %f
  %s6 = add i16 %s5, %g
  ret i16 %s6
}

define i16 @call_sum7(i16 %x) {
  %y = add i16 %x, 1
  %r = call i16 @sum7(i16 %x, i16 %y, i16 %x, i16 %y, i16 %x, i16 %y, i16 %x)
  ret i16 %r
}

; values live across calls need BX, SI, DI (callee-saved, preserved by each callee)
define i16 @pressure(i16 %a, i16 %b, i16 %c) {
  %x = call i16 @id(i16 %a)
  %y = call i16 @id(i16 %b)
  %z = call i16 @id(i16 %c)
  %s1 = add i16 %x, %y
  %s2 = add i16 %s1, %z
  %s3 = add i16 %s2, %a
  %s4 = add i16 %s3, %b
  %s5 = add i16 %s4, %c
  ret i16 %s5
}

define i16 @fib(i16 %n) {
entry:
  %small = icmp ult i16 %n, 2
  br i1 %small, label %base, label %rec
base:
  ret i16 %n
rec:
  %n1 = sub i16 %n, 1
  %n2 = sub i16 %n, 2
  %a = call i16 @fib(i16 %n1)
  %b = call i16 @fib(i16 %n2)
  %r = add i16 %a, %b
  ret i16 %r
}

; 32-bit result in DX:AX
define i32 @make32(i16 %lo, i16 %hi) noinline {
  %l = zext i16 %lo to i32
  %h = zext i16 %hi to i32
  %hs = shl i32 %h, 16
  %r = or i32 %hs, %l
  ret i32 %r
}

define i16 @use32(i16 %a, i16 %b) {
  %v = call i32 @make32(i16 %b, i16 %a)
  %l = trunc i32 %v to i16
  %hs = lshr i32 %v, 16
  %h = trunc i32 %hs to i16
  %r = sub i16 %l, %h
  ret i16 %r
}

; a call whose result is used after another call, in a loop
define i16 @loop_calls(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s1, %loop ]
  %t = call i16 @add3(i16 %i, i16 %s, i16 1)
  %s1 = call i16 @id(i16 %t)
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, %n
  br i1 %more, label %loop, label %exit
exit:
  ret i16 %s1
}
