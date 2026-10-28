; Loads, stores, globals, pointers and stack objects (see arith.ll).

target triple = "i8086"

@counter = global i16 0
@arr = global [8 x i16] [i16 10, i16 20, i16 30, i16 40, i16 50, i16 60, i16 70, i16 80]
@bytes = global [8 x i8] c"\01\02\03\04\05\06\07\08"
@buf = global [16 x i8] zeroinitializer

%rec = type { i16, i8, i16 }

define i16 @get_counter() {
  %v = load i16, ptr @counter
  ret i16 %v
}

define void @set_counter(i16 %v) {
  store i16 %v, ptr @counter
  ret void
}

define i16 @bump_counter(i16 %by) {
  %v = load i16, ptr @counter
  %n = add i16 %v, %by
  store i16 %n, ptr @counter
  ret i16 %n
}

define i16 @arr_get(i16 %i) {
  %p = getelementptr i16, ptr @arr, i16 %i
  %v = load i16, ptr %p
  ret i16 %v
}

define i16 @arr_const() {
  %p = getelementptr i16, ptr @arr, i16 5
  %v = load i16, ptr %p
  ret i16 %v
}

define zeroext i8 @byte_get(i16 %i) {
  %p = getelementptr i8, ptr @bytes, i16 %i
  %v = load i8, ptr %p
  ret i8 %v
}

define i16 @sum_arr(i16 %n) {
entry:
  %empty = icmp eq i16 %n, 0
  br i1 %empty, label %exit, label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s1, %loop ]
  %p = getelementptr i16, ptr @arr, i16 %i
  %v = load i16, ptr %p
  %s1 = add i16 %s, %v
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, %n
  br i1 %more, label %loop, label %exit
exit:
  %r = phi i16 [ 0, %entry ], [ %s1, %loop ]
  ret i16 %r
}

define i16 @ptr_load(ptr %p) {
  %v = load i16, ptr %p
  ret i16 %v
}

define void @ptr_store(ptr %p, i16 %v) {
  store i16 %v, ptr %p
  ret void
}

define zeroext i8 @ptr_load8(ptr %p) {
  %v = load i8, ptr %p
  ret i8 %v
}

define void @ptr_store8(ptr %p, i8 %v) {
  store i8 %v, ptr %p
  ret void
}

define i16 @field(ptr %s) {
  %f = getelementptr %rec, ptr %s, i16 0, i32 2
  %v = load i16, ptr %f
  ret i16 %v
}

define void @set_fields(ptr %s, i16 %a, i8 %b, i16 %c) {
  store i16 %a, ptr %s
  %pb = getelementptr %rec, ptr %s, i16 0, i32 1
  store i8 %b, ptr %pb
  %pc = getelementptr %rec, ptr %s, i16 0, i32 2
  store i16 %c, ptr %pc
  ret void
}

define i16 @swap_add(ptr %p) {
  %q = getelementptr i16, ptr %p, i16 1
  %a = load i16, ptr %p
  %b = load i16, ptr %q
  store i16 %b, ptr %p
  store i16 %a, ptr %q
  %s = add i16 %a, %b
  ret i16 %s
}

define void @fill(ptr %p, i16 %n, i8 %v) {
entry:
  %empty = icmp eq i16 %n, 0
  br i1 %empty, label %exit, label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %q = getelementptr i8, ptr %p, i16 %i
  store i8 %v, ptr %q
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, %n
  br i1 %more, label %loop, label %exit
exit:
  ret void
}

define i16 @buf_checksum() {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i16 [ 0, %entry ], [ %s1, %loop ]
  %p = getelementptr i8, ptr @buf, i16 %i
  %v = load i8, ptr %p
  %w = zext i8 %v to i16
  %s1 = add i16 %s, %w
  %i1 = add i16 %i, 1
  %more = icmp ult i16 %i1, 16
  br i1 %more, label %loop, label %exit
exit:
  ret i16 %s1
}

; a stack object indexed at run time (forces a frame pointer)
define i16 @local_table(i16 %i) {
  %t = alloca [4 x i16], align 2
  store i16 100, ptr %t
  %p1 = getelementptr i16, ptr %t, i16 1
  store i16 200, ptr %p1
  %p2 = getelementptr i16, ptr %t, i16 2
  store i16 300, ptr %p2
  %p3 = getelementptr i16, ptr %t, i16 3
  store i16 400, ptr %p3
  %k = and i16 %i, 3
  %q = getelementptr i16, ptr %t, i16 %k
  %v = load i16, ptr %q
  ret i16 %v
}
