; Several segments: code and data in sections `.far.*` are linked into a segment of
; their own (run.ld). Far functions are called with `call seg:off` and reach
; their data through far pointers made from `name@seg` relocations.

target triple = "i8086"

@fardata = global [4 x i16] [i16 11, i16 22, i16 33, i16 44], section ".far.data"
@farcounter = global i16 0, section ".far.data"

define cc 1000 i16 @far_add(i16 %a, i16 %b) section ".far.text" {
  %s = add i16 %a, %b
  ret i16 %s
}

; the sum of fardata, read through a far pointer to a global in this segment
define cc 1000 i16 @far_sum_data() section ".far.text" {
  %a = load i16, ptr addrspace(1) addrspacecast (ptr @fardata to ptr addrspace(1))
  %p1 = getelementptr i16, ptr addrspace(1) addrspacecast (ptr @fardata to ptr addrspace(1)), i16 1
  %b = load i16, ptr addrspace(1) %p1
  %p2 = getelementptr i16, ptr addrspace(1) addrspacecast (ptr @fardata to ptr addrspace(1)), i16 2
  %c = load i16, ptr addrspace(1) %p2
  %p3 = getelementptr i16, ptr addrspace(1) addrspacecast (ptr @fardata to ptr addrspace(1)), i16 3
  %d = load i16, ptr addrspace(1) %p3
  %s1 = add i16 %a, %b
  %s2 = add i16 %s1, %c
  %s3 = add i16 %s2, %d
  ret i16 %s3
}

define cc 1000 i16 @far_bump(i16 %x) section ".far.text" {
  %c = load i16, ptr addrspace(1) addrspacecast (ptr @farcounter to ptr addrspace(1))
  %n = add i16 %c, %x
  store i16 %n, ptr addrspace(1) addrspacecast (ptr @farcounter to ptr addrspace(1))
  ret i16 %n
}

; a far function calling another one in its own segment, with stack arguments
define cc 1000 i16 @far_chain(i16 %a, i16 %b, i16 %c, i16 %d) section ".far.text" {
  %x = call cc 1000 i16 @far_add(i16 %a, i16 %b)
  %y = call cc 1000 i16 @far_add(i16 %c, i16 %d)
  %z = call cc 1000 i16 @far_add(i16 %x, i16 %y)
  ret i16 %z
}

define i16 @t_add(i16 %a, i16 %b) {
  %r = call cc 1000 i16 @far_add(i16 %a, i16 %b)
  ret i16 %r
}

define i16 @t_sum() {
  %r = call cc 1000 i16 @far_sum_data()
  ret i16 %r
}

define i16 @t_bump(i16 %x) {
  %r = call cc 1000 i16 @far_bump(i16 %x)
  ret i16 %r
}

define i16 @t_chain(i16 %a, i16 %b, i16 %c, i16 %d) {
  %r = call cc 1000 i16 @far_chain(i16 %a, i16 %b, i16 %c, i16 %d)
  ret i16 %r
}

; through a pointer to the far function: the segment half comes from the linker
define i16 @t_pointer(i16 %a, i16 %b) {
  %r = call cc 1000 addrspace(1) i16 addrspacecast (ptr @far_add to ptr addrspace(1))(i16 %a, i16 %b)
  ret i16 %r
}

; the segment half of a pointer to a far global
define i16 @t_segment_of_fardata() {
  %i = ptrtoint ptr addrspace(1) addrspacecast (ptr @fardata to ptr addrspace(1)) to i32
  %s = lshr i32 %i, 16
  %t = trunc i32 %s to i16
  ret i16 %t
}
