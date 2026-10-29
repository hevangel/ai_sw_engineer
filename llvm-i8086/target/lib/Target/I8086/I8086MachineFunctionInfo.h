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
#include "llvm/IR/CallingConv.h"
#include "llvm/IR/Function.h"

namespace llvm {

/// Whether `F` is an interrupt handler: the "interrupt" attribute or the
/// msp430_intrcc calling convention that rustc uses for `extern "msp430-interrupt"`.
inline bool isInterruptHandler(const Function &F) {
  return F.hasFnAttribute("interrupt") ||
         F.getCallingConv() == CallingConv::MSP430_INTR;
}

/// The far calling convention (`cc 1000` in IR): the arguments and results are as
/// for a near call, but the return address is a CS:IP pair (`push cs ; call`
/// to call, `retf` to return), so the stack arguments start two bytes higher.
constexpr CallingConv::ID I8086_FarCC = CallingConv::FirstTargetCC + 936; // 1000, a number LLVM gives no name

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
