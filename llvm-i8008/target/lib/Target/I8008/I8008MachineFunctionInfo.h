//===-- I8008MachineFunctionInfo.h - I8008 machine function info -*- C++ -*-=//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008MACHINEFUNCTIONINFO_H
#define LLVM_LIB_TARGET_I8008_I8008MACHINEFUNCTIONINFO_H

#include "llvm/CodeGen/MachineFunction.h"

namespace llvm {

class I8008MachineFunctionInfo : public MachineFunctionInfo {
  virtual void anchor();

public:
  I8008MachineFunctionInfo() = default;
  I8008MachineFunctionInfo(const Function &F, const TargetSubtargetInfo *STI) {}

  /// Bytes of incoming memory arguments in __args_<fn> (abi.md sec.3.1).
  unsigned ArgAreaSize = 0;

  MachineFunctionInfo *
  clone(BumpPtrAllocator &Allocator, MachineFunction &DestMF,
        const DenseMap<MachineBasicBlock *, MachineBasicBlock *> &Src2DstMBB)
      const override;
};

} // end namespace llvm

#endif
