//===-- I4004RegisterInfo.cpp - I4004 Register Information ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004RegisterInfo.h"
#include "I4004.h"
#include "I4004Subtarget.h"
#include "llvm/ADT/BitVector.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/TargetFrameLowering.h"
#include "llvm/IR/Function.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-reg-info"

#define GET_REGINFO_TARGET_DESC
#include "I4004GenRegisterInfo.inc"

// The "return address" slot concept is the hardware stack; we pass P7 as the
// frame register placeholder (software stack pointer, abi.md sec.4.4).
I4004RegisterInfo::I4004RegisterInfo() : I4004GenRegisterInfo(I4004::P7) {}

const MCPhysReg *
I4004RegisterInfo::getCalleeSavedRegs(const MachineFunction *MF) const {
  // abi.md sec.4.3: P4, P5, P6 are callee-saved.
  static const MCPhysReg CSR[] = {I4004::P4, I4004::P5, I4004::P6, 0};
  return CSR;
}

BitVector I4004RegisterInfo::getReservedRegs(const MachineFunction &MF) const {
  BitVector Reserved(getNumRegs());

  // Fixed hardware resources.
  Reserved.set(I4004::ACC);
  Reserved.set(I4004::CY);

  // P0 (R0:R1): FIN/JIN scratch pointer. P7 (R14:R15): software stack pointer.
  Reserved.set(I4004::P0);
  Reserved.set(I4004::R0);
  Reserved.set(I4004::R1);
  Reserved.set(I4004::P7);
  Reserved.set(I4004::R14);
  Reserved.set(I4004::R15);

  return Reserved;
}

bool I4004RegisterInfo::eliminateFrameIndex(MachineBasicBlock::iterator II,
                                            int SPAdj, unsigned FIOperandNum,
                                            RegScavenger *RS) const {
  // No real stack frames in the milestone-3b backbone (leaf functions only,
  // abi.md sec.4.4 hardware-stack path). A frame index reaching here means a
  // spill the backbone cannot yet place in 4002 RAM; replace the FI operand
  // with the soft-SP register and a zero offset so the verifier is satisfied
  // and the limitation surfaces as wrong runtime behavior, not a crash, until
  // the RAM frame lowering lands.
  MachineInstr &MI = *II;
  int FrameIndex = MI.getOperand(FIOperandNum).getIndex();
  MachineFunction &MF = *MI.getParent()->getParent();
  int Offset = MF.getFrameInfo().getObjectOffset(FrameIndex);
  MI.getOperand(FIOperandNum).ChangeToRegister(I4004::P7, false);
  if (MI.getNumOperands() > FIOperandNum + 1 &&
      MI.getOperand(FIOperandNum + 1).isImm())
    MI.getOperand(FIOperandNum + 1).setImm(Offset);
  return false;
}

Register I4004RegisterInfo::getFrameRegister(const MachineFunction &MF) const {
  return I4004::P7;
}
