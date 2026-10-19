//===-- I4004InstrInfo.h - I4004 Instruction Information -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The I4004 implementation of the TargetInstrInfo class.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004INSTRINFO_H
#define LLVM_LIB_TARGET_I4004_I4004INSTRINFO_H

#include "I4004RegisterInfo.h"
#include "llvm/CodeGen/TargetInstrInfo.h"

#define GET_INSTRINFO_HEADER
#define GET_INSTRINFO_ENUM
#include "I4004GenInstrInfo.inc"

namespace llvm {

class I4004Subtarget;

namespace I4004II {
// Target-specific MachineOperand target flags. Attached to an MBB operand of a
// soft-call FIM so MCInstLower wraps the block symbol in a %lo8/%page specifier
// expression (the software-call return-token materialisation, abi.md 4.4.1).
enum TOF {
  MO_NONE = 0,
  MO_LO8,  // low 8 bits of the symbol (-> R_I4004_LO8)
  MO_PAGE, // page nibble (bits 11..8) of the symbol (-> R_I4004_PAGE4)
};
} // namespace I4004II

class I4004InstrInfo : public I4004GenInstrInfo {
  const I4004RegisterInfo RI;
  virtual void anchor();

public:
  explicit I4004InstrInfo(const I4004Subtarget &STI);

  const I4004RegisterInfo &getRegisterInfo() const { return RI; }

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

  // Branch analysis / editing. Needed by MachineBlockPlacement, BranchFolder
  // and tail-duplication; without them the BranchFolder's tail-merge aborts in
  // ReplaceTailWithBranchTo ("Target didn't implement insertBranch"). We model
  // only the unconditional BR pseudo (-> JUN) precisely; the conditional
  // compare-and-branch pseudos (BRCCi8/BRCCi16, BRZ/BRNZ) are opaque to the
  // generic analysis (their condition is a multi-operand SUB-chain, not a
  // reversible Cond vector), so analyzeBranch reports them unanalyzable and the
  // optimizer leaves them alone.
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

} // end namespace llvm

#endif
