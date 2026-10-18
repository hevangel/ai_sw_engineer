//===-- I4004FrameLowering.cpp - I4004 Frame Lowering --------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004FrameLowering.h"
#include "I4004Subtarget.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"

using namespace llvm;

I4004FrameLowering::I4004FrameLowering(const I4004Subtarget &STI)
    : TargetFrameLowering(TargetFrameLowering::StackGrowsUp, Align(1), 0,
                          Align(1)) {}

bool I4004FrameLowering::hasFPImpl(const MachineFunction &MF) const {
  return false;
}

bool I4004FrameLowering::hasReservedCallFrame(const MachineFunction &MF) const {
  return true;
}

void I4004FrameLowering::emitPrologue(MachineFunction &MF,
                                      MachineBasicBlock &MBB) const {
  // Milestone-3b backbone: leaf functions on the hardware stack need no
  // prologue. A 4002-RAM software frame is a later milestone (abi.md sec.4.4).
}

void I4004FrameLowering::emitEpilogue(MachineFunction &MF,
                                      MachineBasicBlock &MBB) const {
  // See emitPrologue.
}

MachineBasicBlock::iterator I4004FrameLowering::eliminateCallFramePseudoInstr(
    MachineFunction &MF, MachineBasicBlock &MBB,
    MachineBasicBlock::iterator I) const {
  // No call-frame adjustment pseudos are emitted (reserved call frame).
  return MBB.erase(I);
}
