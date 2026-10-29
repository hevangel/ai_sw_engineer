# RUN: not llvm-mc -triple=i8086 %s 2>&1 | FileCheck %s
#
# Diagnostics for what the 8086 cannot do.
  mov ax, bl ; CHECK: error: invalid operand for instruction
  mov [bx], 5 ; CHECK: error: invalid operand for instruction
  mov ax, [sp] ; CHECK: error: only bx, bp, si and di can address memory
  mov ax, [bx+bp] ; CHECK: error: two base registers in memory operand
  mov ax, [si+di] ; CHECK: error: two index registers in memory operand
  shl ax, 2 ; CHECK: error: invalid operand for instruction
  push 5 ; CHECK: error: invalid operand for instruction
  pusha ; CHECK: error: invalid instruction mnemonic
  movzx ax, bl ; CHECK: error: invalid instruction mnemonic
  mov eax, 1 ; CHECK: error: invalid operand for instruction
  pop cs ; CHECK: error: invalid operand for instruction
  mov cs, ax ; CHECK: error: invalid operand for instruction
  add ax, 70000 ; CHECK: error: invalid operand for instruction
  lea ax, bx ; CHECK: error: invalid operand for instruction
  jmp 1, 2 ; CHECK: error: invalid operand for instruction
