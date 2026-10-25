//===-- I8086InstrInfo.h - I8086 Instruction Information --------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086INSTRINFO_H
#define LLVM_LIB_TARGET_I8086_I8086INSTRINFO_H

#include "I8086RegisterInfo.h"
#include "llvm/CodeGen/TargetInstrInfo.h"

#define GET_INSTRINFO_HEADER
#define GET_INSTRINFO_ENUM
#include "I8086GenInstrInfo.inc"

namespace llvm {

class I8086Subtarget;

class I8086InstrInfo : public I8086GenInstrInfo {
  const I8086RegisterInfo RI;
  virtual void anchor();

public:
  explicit I8086InstrInfo(const I8086Subtarget &STI);

  const I8086RegisterInfo &getRegisterInfo() const { return RI; }

  void copyPhysReg(MachineBasicBlock &MBB, MachineBasicBlock::iterator MI,
                   const DebugLoc &DL, Register DestReg, Register SrcReg,
                   bool KillSrc, bool RenamableDest = false,
                   bool RenamableSrc = false) const override;

  void storeRegToStackSlot(MachineBasicBlock &MBB,
                           MachineBasicBlock::iterator MI, Register SrcReg,
                           bool isKill, int FrameIndex,
                           const TargetRegisterClass *RC, Register VReg,
                           MachineInstr::MIFlag Flags =
                               MachineInstr::NoFlags) const override;

  void loadRegFromStackSlot(MachineBasicBlock &MBB,
                            MachineBasicBlock::iterator MI, Register DestReg,
                            int FrameIndex, const TargetRegisterClass *RC,
                            Register VReg, unsigned SubReg = 0,
                            MachineInstr::MIFlag Flags =
                                MachineInstr::NoFlags) const override;

  // Branch analysis models P_JMP and P_JCC; Cond = [cc].
  bool analyzeBranch(MachineBasicBlock &MBB, MachineBasicBlock *&TBB,
                     MachineBasicBlock *&FBB,
                     SmallVectorImpl<MachineOperand> &Cond,
                     bool AllowModify = false) const override;
  unsigned insertBranch(MachineBasicBlock &MBB, MachineBasicBlock *TBB,
                        MachineBasicBlock *FBB, ArrayRef<MachineOperand> Cond,
                        const DebugLoc &DL,
                        int *BytesAdded = nullptr) const override;
  unsigned removeBranch(MachineBasicBlock &MBB,
                        int *BytesRemoved = nullptr) const override;
  bool reverseBranchCondition(
      SmallVectorImpl<MachineOperand> &Cond) const override;
};

/// Condition codes: the low nibble of the Jcc opcode (0x70 + cc).
namespace I8086CC {
enum CondCode : int {
  O = 0, NO = 1, B = 2, AE = 3, E = 4, NE = 5, BE = 6, A = 7,
  S = 8, NS = 9, P = 10, NP = 11, L = 12, GE = 13, LE = 14, G = 15,
};
/// The code that holds exactly when CC does not (flip the low bit).
inline int invert(int CC) { return CC ^ 1; }
} // namespace I8086CC

} // end namespace llvm

#endif
