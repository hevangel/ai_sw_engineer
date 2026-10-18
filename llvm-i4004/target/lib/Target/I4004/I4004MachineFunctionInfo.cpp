//===-- I4004MachineFunctionInfo.cpp - I4004 machine function info -------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004MachineFunctionInfo.h"

using namespace llvm;

void I4004MachineFunctionInfo::anchor() {}

MachineFunctionInfo *I4004MachineFunctionInfo::clone(
    BumpPtrAllocator &Allocator, MachineFunction &DestMF,
    const DenseMap<MachineBasicBlock *, MachineBasicBlock *> &Src2DstMBB)
    const {
  return DestMF.cloneInfo<I4004MachineFunctionInfo>(*this);
}
