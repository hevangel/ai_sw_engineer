//===-- I4004FrameLowering.h - Define frame lowering for I4004 -*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Trivial frame lowering for the milestone-3b backbone: leaf functions on the
// 3-level hardware return stack emit no prologue/epilogue (abi.md sec.4.4). A
// software return stack in 4002 RAM is a later milestone.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004FRAMELOWERING_H
#define LLVM_LIB_TARGET_I4004_I4004FRAMELOWERING_H

#include "llvm/CodeGen/TargetFrameLowering.h"

namespace llvm {

class I4004Subtarget;

class I4004FrameLowering : public TargetFrameLowering {
protected:
  bool hasFPImpl(const MachineFunction &MF) const override;

public:
  explicit I4004FrameLowering(const I4004Subtarget &STI);

  void emitPrologue(MachineFunction &MF, MachineBasicBlock &MBB) const override;
  void emitEpilogue(MachineFunction &MF, MachineBasicBlock &MBB) const override;

  bool hasReservedCallFrame(const MachineFunction &MF) const override;

  MachineBasicBlock::iterator
  eliminateCallFramePseudoInstr(MachineFunction &MF, MachineBasicBlock &MBB,
                                MachineBasicBlock::iterator I) const override;
};

} // end namespace llvm

#endif
