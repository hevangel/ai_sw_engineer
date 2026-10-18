//===-- I4004MachineFunctionInfo.h - I4004 machine function info -*- C++ -*-=//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004MACHINEFUNCTIONINFO_H
#define LLVM_LIB_TARGET_I4004_I4004MACHINEFUNCTIONINFO_H

#include "llvm/CodeGen/MachineFunction.h"

namespace llvm {

class I4004MachineFunctionInfo : public MachineFunctionInfo {
  virtual void anchor();

public:
  I4004MachineFunctionInfo() = default;
  I4004MachineFunctionInfo(const Function &F, const TargetSubtargetInfo *STI) {}

  MachineFunctionInfo *
  clone(BumpPtrAllocator &Allocator, MachineFunction &DestMF,
        const DenseMap<MachineBasicBlock *, MachineBasicBlock *> &Src2DstMBB)
      const override;
};

} // end namespace llvm

#endif
