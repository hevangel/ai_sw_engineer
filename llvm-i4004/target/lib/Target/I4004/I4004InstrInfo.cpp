//===-- I4004InstrInfo.cpp - I4004 Instruction Information ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004InstrInfo.h"
#include "I4004.h"
#include "I4004Subtarget.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"

using namespace llvm;

#define GET_INSTRINFO_CTOR_DTOR
#include "I4004GenInstrInfo.inc"

void I4004InstrInfo::anchor() {}

I4004InstrInfo::I4004InstrInfo(const I4004Subtarget &STI)
    : I4004GenInstrInfo(STI, RI, I4004::ADJCALLSTACKDOWN, I4004::ADJCALLSTACKUP),
      RI() {}

void I4004InstrInfo::copyPhysReg(MachineBasicBlock &MBB,
                                 MachineBasicBlock::iterator MI,
                                 const DebugLoc &DL, Register DestReg,
                                 Register SrcReg, bool KillSrc,
                                 bool RenamableDest, bool RenamableSrc) const {
  // Emit the real nibble moves directly (LD src; XCH dst, per nibble). We do
  // NOT use a pseudo here because copyPhysReg can be invoked by passes that run
  // after I4004ExpandPseudos (e.g. post-RA copy lowering), which would leave an
  // unexpanded pseudo for the AsmPrinter.
  auto copyPair = [&](Register D, Register S) {
    Register DHi = RI.getSubReg(D, I4004::sub_hi);
    Register DLo = RI.getSubReg(D, I4004::sub_lo);
    Register SHi = RI.getSubReg(S, I4004::sub_hi);
    Register SLo = RI.getSubReg(S, I4004::sub_lo);
    BuildMI(MBB, MI, DL, get(I4004::LD)).addReg(SHi);
    BuildMI(MBB, MI, DL, get(I4004::XCH)).addReg(DHi, RegState::Define);
    BuildMI(MBB, MI, DL, get(I4004::LD)).addReg(SLo);
    BuildMI(MBB, MI, DL, get(I4004::XCH)).addReg(DLo, RegState::Define);
  };

  if (I4004::DR16RegClass.contains(DestReg, SrcReg)) {
    copyPair(RI.getSubReg(DestReg, I4004::sub_pair_lo),
             RI.getSubReg(SrcReg, I4004::sub_pair_lo));
    copyPair(RI.getSubReg(DestReg, I4004::sub_pair_hi),
             RI.getSubReg(SrcReg, I4004::sub_pair_hi));
    return;
  }
  copyPair(DestReg, SrcReg);
}

void I4004InstrInfo::storeRegToStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register SrcReg,
    bool isKill, int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    MachineInstr::MIFlag Flags) const {
  // Spill to the 4002-RAM spill region (abi.md sec.2.1). The frame index is
  // carried on a SPILL pseudo and turned into a concrete RAM address by the
  // post-RA expander (I4004ExpandPseudos). DR16 spills as two i8 halves.
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  if (I4004::DR16RegClass.hasSubClassEq(RC)) {
    BuildMI(MBB, MI, DL, get(I4004::SPILL16))
        .addReg(SrcReg, getKillRegState(isKill))
        .addFrameIndex(FrameIndex);
  } else {
    BuildMI(MBB, MI, DL, get(I4004::SPILL8))
        .addReg(SrcReg, getKillRegState(isKill))
        .addFrameIndex(FrameIndex);
  }
}

void I4004InstrInfo::loadRegFromStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register DestReg,
    int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    unsigned SubReg, MachineInstr::MIFlag Flags) const {
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  if (I4004::DR16RegClass.hasSubClassEq(RC)) {
    BuildMI(MBB, MI, DL, get(I4004::RELOAD16), DestReg)
        .addFrameIndex(FrameIndex);
  } else {
    BuildMI(MBB, MI, DL, get(I4004::RELOAD8), DestReg).addFrameIndex(FrameIndex);
  }
}

//===----------------------------------------------------------------------===//
// Branch analysis / editing.
//
// The target's terminators are:
//   BR <mbb>                  - unconditional (-> JUN), analyzable + editable.
//   BRCCi8/BRCCi16 ...,<mbb>   - conditional compare-and-branch (-> SUB + JCN).
//   BRZ/BRNZ ...,<mbb>         - conditional zero-test branch.
// Only BR is representable as a generic (TBB, FBB, Cond) tuple, because the
// conditional pseudos carry their compare operands inline and have no
// reversible i8 condition vector. So analyzeBranch handles the unconditional
// and fall-through shapes and the "ends with a conditional pseudo we don't
// model" shapes it reports as unanalyzable (return true). That is enough for
// MachineBlockPlacement / BranchFolder, which only need to insert and remove
// UNCONDITIONAL branches when they rearrange blocks.
//===----------------------------------------------------------------------===//

bool I4004InstrInfo::analyzeBranch(MachineBasicBlock &MBB,
                                   MachineBasicBlock *&TBB,
                                   MachineBasicBlock *&FBB,
                                   SmallVectorImpl<MachineOperand> &Cond,
                                   bool AllowModify) const {
  TBB = FBB = nullptr;
  Cond.clear();

  MachineBasicBlock::iterator I = MBB.getLastNonDebugInstr();
  if (I == MBB.end() || !isUnpredicatedTerminator(*I))
    return false; // falls through; no terminator to analyze.

  // Walk back over the terminators. If we see any terminator other than the
  // unconditional BR we cannot represent, bail (unanalyzable).
  MachineBasicBlock::iterator FirstTerm = I;
  while (FirstTerm != MBB.begin()) {
    auto Prev = std::prev(FirstTerm);
    if (!isUnpredicatedTerminator(*Prev))
      break;
    FirstTerm = Prev;
  }

  for (auto T = FirstTerm; T != MBB.end(); ++T) {
    if (T->getOpcode() == I4004::BR) {
      // Unconditional branch: its target is the (new) TBB. A BR must be the
      // last terminator; anything after it is dead.
      TBB = T->getOperand(0).getMBB();
      continue;
    }
    // A conditional pseudo (BRCC*/BRZ/BRNZ) or anything else: not modeled.
    return true;
  }

  // Only an unconditional BR (or none) remained. If we found one, TBB is set
  // and Cond is empty (unconditional). Fall-through otherwise.
  return false;
}

unsigned I4004InstrInfo::insertBranch(MachineBasicBlock &MBB,
                                      MachineBasicBlock *TBB,
                                      MachineBasicBlock *FBB,
                                      ArrayRef<MachineOperand> Cond,
                                      const DebugLoc &DL,
                                      int *BytesAdded) const {
  assert(TBB && "insertBranch must have a true destination");
  // We only ever model unconditional branches (Cond empty, no FBB). A target
  // that reports its conditional branches unanalyzable is never asked to
  // re-insert them, so a non-empty Cond here would be a logic error.
  assert(Cond.empty() && !FBB &&
         "i4004 only inserts unconditional branches (conditionals are opaque)");
  BuildMI(&MBB, DL, get(I4004::BR)).addMBB(TBB);
  if (BytesAdded)
    *BytesAdded = 2; // JUN is a two-byte instruction.
  return 1;
}

unsigned I4004InstrInfo::removeBranch(MachineBasicBlock &MBB,
                                      int *BytesRemoved) const {
  unsigned Count = 0;
  int Bytes = 0;
  MachineBasicBlock::iterator I = MBB.end();
  while (I != MBB.begin()) {
    --I;
    if (I->isDebugInstr())
      continue;
    // Only remove branches we model; stop at the first non-(modeled)-terminator.
    if (I->getOpcode() != I4004::BR)
      break;
    Bytes += 2;
    I->eraseFromParent();
    I = MBB.end();
    ++Count;
  }
  if (BytesRemoved)
    *BytesRemoved = Bytes;
  return Count;
}
