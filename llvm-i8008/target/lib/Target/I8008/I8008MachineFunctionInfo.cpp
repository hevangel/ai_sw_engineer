//===-- I8008MachineFunctionInfo.cpp - I8008 machine function info -------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008MachineFunctionInfo.h"

using namespace llvm;

void I8008MachineFunctionInfo::anchor() {}

MachineFunctionInfo *I8008MachineFunctionInfo::clone(
    BumpPtrAllocator &Allocator, MachineFunction &DestMF,
    const DenseMap<MachineBasicBlock *, MachineBasicBlock *> &Src2DstMBB)
    const {
  return DestMF.cloneInfo<I8008MachineFunctionInfo>(*this);
}
