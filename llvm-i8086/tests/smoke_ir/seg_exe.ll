; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; CHECK: mov ax, msg_far@seg
; CHECK: call far_print@seg:far_print
; A DOS .EXE with two segments (system/ibm_pc/platform/link_exe_seg.ld). The first
; segment is the program segment (offset 100h up): the entry point, a message and the
; stack. The second holds a far function and a message the far function prints with
; INT 21h function 9, setting DS to the segment the linker gave `msg_far`
; (`msg_far@seg`, relocated by DOS when it loads the file).

target triple = "i8086"

@msg_near = global [13 x i8] c"Near again\0D\0A$"
@msg_far = global [15 x i8] c"Far segment!\0D\0A$", section ".far.data"
@stack = global [1024 x i8] zeroinitializer
@__stack_top = alias i8, getelementptr (i8, ptr @stack, i16 1024)

define cc 1000 void @far_print() section ".far.text" {
  call void asm sideeffect "push ds\0A\09mov ax, msg_far@seg\0A\09mov ds, ax\0A\09mov dx, msg_far\0A\09mov ah, 9\0A\09int 0x21\0A\09pop ds", "~{ax},~{dx},~{memory}"()
  ret void
}

define cc 1000 i16 @far_twice(i16 %x) section ".far.text" {
  %r = shl i16 %x, 1
  ret i16 %r
}

define void @_start() section ".text.start" {
  call cc 1000 void @far_print()
  %r = call cc 1000 i16 @far_twice(i16 21)
  ; 42 is '*'
  call void asm sideeffect "mov ah, 2\0A\09int 0x21", "{dx},~{ax}"(i16 %r)
  call void asm sideeffect "mov ah, 9\0A\09int 0x21", "{dx},~{ax}"(ptr @msg_near)
  call void asm sideeffect "int 0x20", ""()
  unreachable
}
