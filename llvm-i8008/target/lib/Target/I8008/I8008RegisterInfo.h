//===-- I8008RegisterInfo.h - I8008 Register Information -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008REGISTERINFO_H
#define LLVM_LIB_TARGET_I8008_I8008REGISTERINFO_H

#include "llvm/CodeGen/TargetRegisterInfo.h"

#define GET_REGINFO_HEADER
#define GET_REGINFO_ENUM
#include "I8008GenRegisterInfo.inc"

namespace llvm {

class I8008RegisterInfo : public I8008GenRegisterInfo {
public:
  I8008RegisterInfo();

  const MCPhysReg *getCalleeSavedRegs(const MachineFunction *MF) const override;
  const uint32_t *getCallPreservedMask(const MachineFunction &MF,
                                       CallingConv::ID) const override;
  BitVector getReservedRegs(const MachineFunction &MF) const override;
  const TargetRegisterClass *
  getPointerRegClass(unsigned Kind = 0) const override;
  bool eliminateFrameIndex(MachineBasicBlock::iterator II, int SPAdj,
                           unsigned FIOperandNum,
                           RegScavenger *RS = nullptr) const override;
  Register getFrameRegister(const MachineFunction &MF) const override;

  /// Never fold an 8-bit value into half of a 16-bit virtual register: with
  /// only B C D E, an instruction could then need two pairs plus a byte, which
  /// the allocator cannot satisfy. Such copies stay one-byte register moves.
  bool shouldCoalesce(MachineInstr *MI, const TargetRegisterClass *SrcRC,
                      unsigned SubReg, const TargetRegisterClass *DstRC,
                      unsigned DstSubReg, const TargetRegisterClass *NewRC,
                      LiveIntervals &LIS) const override {
    return SubReg == 0 && DstSubReg == 0;
  }

  /// Preserved by a `__leaf_*` call that returns a value / returns nothing.
  static const uint32_t *leafMask();
  static const uint32_t *leafVoidMask();

  /// "__frame_<fn>", owned by the MCContext (module lifetime).
  static const char *frameSymbol(const MachineFunction &MF);
};

} // end namespace llvm

#endif
