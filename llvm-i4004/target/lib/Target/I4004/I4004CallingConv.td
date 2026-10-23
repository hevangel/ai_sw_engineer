//===-- I4004CallingConv.td - Calling Conventions for I4004 -*- tablegen -*-==//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The "i4004-C" calling convention (spec/abi.md sec.4).
//
//  * First three i8/pointer args in pairs P1, P2, P3.
//  * i8 return value in P1.
//  * i16 is split into two i8s by the legalizer, so it consumes two pairs /
//    two return slots (P1:P2) via the same i8 rules.
//  * Further args spill to the stack (modeled as 4002 RAM by the backend).
//
// P0 is never an argument register (FIN/JIN scratch); P7 is the software SP.
//
//===----------------------------------------------------------------------===//

def CC_I4004 : CallingConv<[
  // Promote i1/i4 to i8.
  CCIfType<[i1], CCPromoteToType<i8>>,

  // i8 (and legalized i16 halves) go in P1..P3.
  CCIfType<[i8], CCAssignToReg<[P1, P2, P3]>>,

  // i16 / pointers go in a double-pair D1 or D2 (D0 holds reserved P0, D3 holds
  // reserved P7). abi.md sec.4.1: a 16-bit handle takes two pairs.
  CCIfType<[i16], CCAssignToReg<[D1, D2]>>,

  // Everything else: byte slot(s) on the (RAM-backed) stack.
  CCIfType<[i8], CCAssignToStack<1, 1>>,
  CCIfType<[i16], CCAssignToStack<2, 1>>
]>;

def RetCC_I4004 : CallingConv<[
  CCIfType<[i1], CCPromoteToType<i8>>,
  // i8 return in P1; i16 return in D1 (abi.md sec.4.2).
  CCIfType<[i8], CCAssignToReg<[P1, P2]>>,
  CCIfType<[i16], CCAssignToReg<[D1, D2]>>
]>;

// Callee-saved pairs (abi.md sec.4.3): P6 only. P4:P5 (double-pair D2) is a
// caller-saved i16 argument/return pair -- a `{ i16, ptr }` return lands its
// second word there -- so it cannot also be callee-saved.
def CSR_I4004 : CalleeSavedRegs<(add P6)>;

// Hand-written leaf routines named `__leaf_*` (and the platform HAL's
// `__bus_*`) touch only P0-P3, ACC and CY, so a call to one also preserves
// P4:P5 (issue #18: values live across the hot digit-loop calls stay in
// registers instead of being spilled). Selected per call site in LowerCall.
def CSR_I4004_Leaf : CalleeSavedRegs<(add P4, P5, P6)>;
