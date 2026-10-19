//===-- I4004RegisterInfo.h - I4004 Register Information -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The I4004 implementation of the TargetRegisterInfo class.
//
// Reserved per spec/abi.md sec.2: P0 (R0:R1, FIN/JIN scratch), P7 (R14:R15,
// software stack pointer), and the fixed ACC/CY. The general allocatable pool
// is R2..R13 / P1..P6.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004REGISTERINFO_H
#define LLVM_LIB_TARGET_I4004_I4004REGISTERINFO_H

#include "llvm/CodeGen/TargetRegisterInfo.h"

#define GET_REGINFO_HEADER
#define GET_REGINFO_ENUM
#include "I4004GenRegisterInfo.inc"

namespace llvm {

class I4004RegisterInfo : public I4004GenRegisterInfo {
public:
  I4004RegisterInfo();

  const MCPhysReg *getCalleeSavedRegs(const MachineFunction *MF) const override;
  BitVector getReservedRegs(const MachineFunction &MF) const override;

  const TargetRegisterClass *
  getPointerRegClass(unsigned Kind = 0) const override;

  bool eliminateFrameIndex(MachineBasicBlock::iterator II, int SPAdj,
                           unsigned FIOperandNum,
                           RegScavenger *RS = nullptr) const override;

  Register getFrameRegister(const MachineFunction &MF) const override;
};

} // end namespace llvm

#endif
