//===-- I8008FrameLowering.cpp - I8008 Frame Lowering --------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008FrameLowering.h"
#include "I8008Subtarget.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"

using namespace llvm;

I8008FrameLowering::I8008FrameLowering(const I8008Subtarget &STI)
    : TargetFrameLowering(TargetFrameLowering::StackGrowsUp, Align(1), 0,
                          Align(1)) {}

bool I8008FrameLowering::hasFPImpl(const MachineFunction &MF) const {
  return false;
}

bool I8008FrameLowering::hasReservedCallFrame(const MachineFunction &MF) const {
  return true;
}

void I8008FrameLowering::emitPrologue(MachineFunction &MF,
                                      MachineBasicBlock &MBB) const {
  // Static frames (abi.md sec.4): nothing to set up.
}

void I8008FrameLowering::emitEpilogue(MachineFunction &MF,
                                      MachineBasicBlock &MBB) const {
  // See emitPrologue.
}

MachineBasicBlock::iterator I8008FrameLowering::eliminateCallFramePseudoInstr(
    MachineFunction &MF, MachineBasicBlock &MBB,
    MachineBasicBlock::iterator I) const {
  // No call-frame adjustment pseudos are emitted (reserved call frame).
  return MBB.erase(I);
}
