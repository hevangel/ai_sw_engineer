# RUN: llvm-mc -triple=i8086 -filetype=obj %s -o %t.o
# RUN: llvm-objdump -d -r %t.o | FileCheck %s
#
# Branch relaxation (spec/asm_syntax.md): a bare jmp/jcc starts short and grows
# to the near form (a conditional jump to "j!cc +3 ; jmp near") when the
# displacement does not fit a signed byte or the target is a symbol the linker
# places; "short" and loop/jcxz never relax.
  .text
start:
  je  far_label ; in range of nothing: relaxed
  jmp far_label
  loop start
  jcxz start
  .space 200
far_label:
  jmp start ; backwards, 0xd8 bytes: near
  jne start
  je  skip ; forward 2 bytes: stays short
  nop
  nop
skip:
  call ext
  jmp ext
  je  ext
  jmp short skip

# CHECK:       0: 75 03 jne short 0x5
# CHECK-NEXT:  2: e9 {{[0-9a-f]+}} {{[0-9a-f ]+}}jmp near
# CHECK-NEXT:  5: e9 {{.*}}jmp near
# CHECK-NEXT:  8: e2 {{.*}}loop 0x0
# CHECK-NEXT:  a: e3 {{.*}}jcxz 0x0
# CHECK:       jmp near 0x0
# CHECK:       je short
# CHECK:       jmp near 0x0
# CHECK:       je short 0x
# CHECK:       call
# CHECK-NEXT:  R_I8086_PC16 ext-0x2
# CHECK:       jmp near
# CHECK-NEXT:  R_I8086_PC16 ext-0x2
# CHECK:       jne short
# CHECK-NEXT:  jmp near
# CHECK-NEXT:  R_I8086_PC16 ext-0x2
