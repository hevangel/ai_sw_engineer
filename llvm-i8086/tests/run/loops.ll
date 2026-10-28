; Loops with counters (loop in small functions) and branches on a result's zero
; flag. See arith.ll for how this file is used.

target triple = "i8086"


; a small counted loop: `loop`
define i16 @sum_down(ptr %p, i16 %n) {
entry:
  br label %l
l:
  %i = phi i16 [ %n, %entry ], [ %i2, %l ]
  %acc = phi i16 [ 0, %entry ], [ %a2, %l ]
  %q = getelementptr i16, ptr %p, i16 %i
  %v = load i16, ptr %q
  %a2 = add i16 %acc, %v
  %i2 = sub i16 %i, 1
  %c = icmp ne i16 %i2, 0
  br i1 %c, label %l, label %out
out:
  ret i16 %a2
}

; a counted loop with a call inside: the counter must survive the call
define i16 @calls_in_loop(i16 %n) {
entry:
  br label %l
l:
  %i = phi i16 [ %n, %entry ], [ %i2, %l ]
  %acc = phi i16 [ 0, %entry ], [ %a2, %l ]
  %v = call i16 @helper(i16 %i)
  %a2 = add i16 %acc, %v
  %i2 = sub i16 %i, 1
  %c = icmp ne i16 %i2, 0
  br i1 %c, label %l, label %out
out:
  ret i16 %a2
}

define i16 @helper(i16 %x) {
  %y = mul i16 %x, 3
  %z = add i16 %y, 1
  ret i16 %z
}

; a longer function with a counted loop: out of `loop` range by design, plain dec/jne
define i16 @long_loop(i16 %n, i16 %a, i16 %b, i16 %c) {
entry:
  br label %l
l:
  %i = phi i16 [ %n, %entry ], [ %i2, %l ]
  %x = phi i16 [ %a, %entry ], [ %x6, %l ]
  %x1 = add i16 %x, %b
  %x2 = xor i16 %x1, %c
  %x3 = shl i16 %x2, 1
  %x4 = add i16 %x3, %a
  %x5 = and i16 %x4, 4095
  %x6 = or i16 %x5, 3
  %i2 = sub i16 %i, 1
  %t = icmp ne i16 %i2, 0
  br i1 %t, label %l, label %out
out:
  ret i16 %x6
}

; branches on the zero flag of the instruction before
define i16 @z_sub(i16 %a, i16 %b) {
  %d = sub i16 %a, %b
  %c = icmp eq i16 %d, 0
  %r = select i1 %c, i16 111, i16 222
  ret i16 %r
}
define i16 @z_and(i16 %a, i16 %b) {
  %d = and i16 %a, %b
  %c = icmp ne i16 %d, 0
  %r = select i1 %c, i16 333, i16 444
  ret i16 %r
}
define i16 @z_xor(i16 %a, i16 %b) {
  %d = xor i16 %a, %b
  %c = icmp eq i16 %d, 0
  br i1 %c, label %y, label %n
y:
  ret i16 1
n:
  ret i16 %d
}
define i16 @z_or(i16 %a, i16 %b) {
  %d = or i16 %a, %b
  %c = icmp eq i16 %d, 0
  br i1 %c, label %y, label %n
y:
  ret i16 5
n:
  ret i16 %d
}
define i16 @z_dec(i16 %a) {
  %d = add i16 %a, -1
  %c = icmp eq i16 %d, 0
  br i1 %c, label %y, label %n
y:
  ret i16 7
n:
  ret i16 %d
}
define i16 @z_neg(i16 %a) {
  %d = sub i16 0, %a
  %c = icmp ne i16 %d, 0
  br i1 %c, label %y, label %n
y:
  ret i16 %d
n:
  ret i16 9
}
