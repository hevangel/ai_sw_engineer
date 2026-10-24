//===-- I8008CallingConv.td - Calling Conventions for I8008 -*- tablegen -*-==//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// spec/abi.md sec.3. CCState marks every alias of an assigned register, so
// an i8 in B makes BC unavailable for a later i16 and vice versa.
//
//===----------------------------------------------------------------------===//

def CC_I8008 : CallingConv<[
  CCIfType<[i1], CCPromoteToType<i8>>,
  CCIfType<[i8], CCAssignToReg<[B, C, D, E]>>,
  CCIfType<[i16], CCAssignToReg<[BC, DE]>>,
  // Memory arguments: __args_<callee> + offset (abi.md sec.3.1).
  CCIfType<[i8], CCAssignToStack<1, 1>>,
  CCIfType<[i16], CCAssignToStack<2, 1>>
]>;

def RetCC_I8008 : CallingConv<[
  CCIfType<[i1], CCPromoteToType<i8>>,
  CCIfType<[i8], CCAssignToReg<[B, C, D, E]>>,
  CCIfType<[i16], CCAssignToReg<[BC, DE]>>
]>;

// No callee-saved registers (abi.md sec.3.3).
def CSR_I8008 : CalleeSavedRegs<(add)>;
