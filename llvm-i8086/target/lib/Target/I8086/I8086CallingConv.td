//===-- I8086CallingConv.td - Calling Conventions for I8086 -*- tablegen -*-==//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// spec/abi.md D3. Bytes travel as 16-bit values. The first three 16-bit
// arguments are in AX, DX, CX; the rest are pushed right to left in 2-byte
// slots. A 32-bit value is two 16-bit pieces, low first, taken from the same
// sequence (returns: DX:AX).
//
//===----------------------------------------------------------------------===//

def CC_I8086 : CallingConv<[
  CCIfType<[i1, i8], CCPromoteToType<i16>>,
  CCIfType<[i16], CCAssignToReg<[AX, DX, CX]>>,
  CCIfType<[i16], CCAssignToStack<2, 2>>
]>;

def RetCC_I8086 : CallingConv<[
  CCIfType<[i1, i8], CCPromoteToType<i16>>,
  CCIfType<[i16], CCAssignToReg<[AX, DX]>>
]>;

// BX SI DI BP survive a call (abi.md D3). BP is the frame pointer and is
// saved by the prologue itself, not through the callee-saved machinery.
def CSR_I8086 : CalleeSavedRegs<(add BX, SI, DI, BP)>;
