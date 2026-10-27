; Far data pointers (address space 1: segment in the high word, offset in the low
; word; spec/abi.md D7). See arith.ll for how this file is used.

target triple = "i8086"

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1), ptr addrspace(1), i16, i1)
declare void @llvm.memcpy.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)
declare void @llvm.memset.p1.i16(ptr addrspace(1), i8, i16, i1)

define i8 @load8(ptr addrspace(1) %p) {
  %v = load i8, ptr addrspace(1) %p
  ret i8 %v
}
define i16 @load16(ptr addrspace(1) %p) {
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}
define i32 @load32(ptr addrspace(1) %p) {
  %v = load i32, ptr addrspace(1) %p
  ret i32 %v
}
define i64 @load64(ptr addrspace(1) %p) {
  %v = load i64, ptr addrspace(1) %p
  ret i64 %v
}
define void @store8(ptr addrspace(1) %p, i8 %v) {
  store i8 %v, ptr addrspace(1) %p
  ret void
}
define void @store16(ptr addrspace(1) %p, i16 %v) {
  store i16 %v, ptr addrspace(1) %p
  ret void
}
define void @store32(ptr addrspace(1) %p, i32 %v) {
  store i32 %v, ptr addrspace(1) %p
  ret void
}

; element i of a word array: the offset changes, the segment does not (and wraps)
define ptr addrspace(1) @elem16(ptr addrspace(1) %p, i16 %i) {
  %q = getelementptr i16, ptr addrspace(1) %p, i16 %i
  ret ptr addrspace(1) %q
}
define i16 @load_elem16(ptr addrspace(1) %p, i16 %i) {
  %q = getelementptr i16, ptr addrspace(1) %p, i16 %i
  %v = load i16, ptr addrspace(1) %q
  ret i16 %v
}

; a far pointer from its two halves
define ptr addrspace(1) @make(i16 %seg, i16 %off) {
  %s = zext i16 %seg to i32
  %h = shl i32 %s, 16
  %o = zext i16 %off to i32
  %v = or i32 %h, %o
  %p = inttoptr i32 %v to ptr addrspace(1)
  ret ptr addrspace(1) %p
}

; the sum of n words, in a loop
define i16 @sum16(ptr addrspace(1) %p, i16 %n) {
entry:
  br label %l
l:
  %i = phi i16 [ 0, %entry ], [ %i2, %l ]
  %acc = phi i16 [ 0, %entry ], [ %a2, %l ]
  %q = getelementptr i16, ptr addrspace(1) %p, i16 %i
  %v = load i16, ptr addrspace(1) %q
  %a2 = add i16 %acc, %v
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, %n
  br i1 %c, label %l, label %out
out:
  ret i16 %a2
}

define void @far_copy(ptr addrspace(1) %d, ptr addrspace(1) %s, i16 %n) {
  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) %d, ptr addrspace(1) %s, i16 %n, i1 false)
  ret void
}
define void @far_to_near(ptr %d, ptr addrspace(1) %s, i16 %n) {
  call void @llvm.memcpy.p0.p1.i16(ptr %d, ptr addrspace(1) %s, i16 %n, i1 false)
  ret void
}
define void @far_set(ptr addrspace(1) %d, i8 %v, i16 %n) {
  call void @llvm.memset.p1.i16(ptr addrspace(1) %d, i8 %v, i16 %n, i1 false)
  ret void
}

; a near pointer as a far one (the segment is DS) and back
@near_value = global i16 4660
define i16 @near_as_far() {
  %p = addrspacecast ptr @near_value to ptr addrspace(1)
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}
define i16 @far_as_near(ptr addrspace(1) %p) {
  %q = addrspacecast ptr addrspace(1) %p to ptr
  %v = load i16, ptr %q
  ret i16 %v
}

; comparisons and selects of far pointers
define i16 @far_eq(ptr addrspace(1) %a, ptr addrspace(1) %b) {
  %c = icmp eq ptr addrspace(1) %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}
