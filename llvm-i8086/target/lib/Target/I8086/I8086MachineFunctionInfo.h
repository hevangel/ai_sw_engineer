//===-- I8086MachineFunctionInfo.h - I8086 machine function info -*- C++ -*-=//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086MACHINEFUNCTIONINFO_H
#define LLVM_LIB_TARGET_I8086_I8086MACHINEFUNCTIONINFO_H

#include "llvm/CodeGen/MachineFunction.h"

namespace llvm {

class I8086MachineFunctionInfo : public MachineFunctionInfo {
  virtual void anchor();

public:
  I8086MachineFunctionInfo() = default;
  I8086MachineFunctionInfo(const Function &F, const TargetSubtargetInfo *STI) {}

  /// Whether the function addresses its frame through BP. Unknown (-1) until
  /// the frame lowering has seen the final stack objects, and then fixed, so
  /// the callee-saved slots it creates cannot change the answer.
  int HasFP = -1;
  /// Callee-saved registers the prologue pushes after BP (BX, SI, DI).
  unsigned NumPushed = 0;
  /// Fixed stack object at the first variadic argument (va_start).
  int VarArgsFrameIndex = 0;

  MachineFunctionInfo *
  clone(BumpPtrAllocator &Allocator, MachineFunction &DestMF,
        const DenseMap<MachineBasicBlock *, MachineBasicBlock *> &Src2DstMBB)
      const override;
};

} // end namespace llvm

#endif
