//===-- I4004InstrInfo.cpp - I4004 Instruction Information ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004InstrInfo.h"
#include "I4004.h"
#include "I4004ISelLowering.h"
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

  // Find the first terminator in the run of trailing terminators.
  MachineBasicBlock::iterator FirstTerm = I;
  while (FirstTerm != MBB.begin()) {
    auto Prev = std::prev(FirstTerm);
    if (!isUnpredicatedTerminator(*Prev))
      break;
    FirstTerm = Prev;
  }

  // Classify each terminator. We model two conditional shapes exactly so the
  // generic BranchFolder / MachineBlockPlacement keep the CFG consistent (an
  // unmodelled conditional branch let BlockPlacement delete a block still
  // targeted by a JCN, producing dangling .LBB labels):
  //   BRCCi8/BRCCi16 <lhs>,<rhs>,<cc>,<tgt>   -> conditional
  //   BR <tgt>                                -> unconditional
  // Cond encoding: [opcode, ccimm, lhs, rhs] (4 operands).
  auto isCondBr = [](unsigned Op) {
    return Op == I4004::BRCCi8 || Op == I4004::BRCCi16;
  };

  for (auto T = FirstTerm; T != MBB.end(); ++T) {
    unsigned Op = T->getOpcode();
    if (T->isDebugInstr())
      continue;
    if (Op == I4004::BR) {
      // Unconditional branch. If a conditional was already recorded, this is
      // the false-path target (FBB); otherwise it is TBB.
      if (Cond.empty())
        TBB = T->getOperand(0).getMBB();
      else
        FBB = T->getOperand(0).getMBB();
      continue;
    }
    if (isCondBr(Op)) {
      // Only one conditional branch per block is modelled.
      if (!Cond.empty())
        return true;
      // Operands: lhs(0), rhs(1), cc-imm(2), target MBB(3).
      TBB = T->getOperand(3).getMBB();
      Cond.push_back(MachineOperand::CreateImm(Op));               // [0] opcode
      Cond.push_back(MachineOperand::CreateImm(T->getOperand(2).getImm())); // [1] cc
      Cond.push_back(T->getOperand(0));                            // [2] lhs
      Cond.push_back(T->getOperand(1));                            // [3] rhs
      continue;
    }
    // Any other terminator (BRZ/BRNZ, indirect, etc.): not modelled.
    return true;
  }

  // Consistency: a recorded conditional with no explicit BR falls through to
  // the layout successor (FBB stays null, which the generic code reads as
  // "fall through"). Pure-unconditional or fall-through are already set.
  return false;
}

bool I4004InstrInfo::reverseBranchCondition(
    SmallVectorImpl<MachineOperand> &Cond) const {
  if (Cond.size() != 4)
    return true; // cannot reverse
  // Cond[1] is the I4004CC code; flip to its complement.
  int CC = Cond[1].getImm();
  int Rev;
  switch (CC) {
  case I4004CC::EQ:  Rev = I4004CC::NE;  break;
  case I4004CC::NE:  Rev = I4004CC::EQ;  break;
  case I4004CC::ULT: Rev = I4004CC::UGE; break;
  case I4004CC::UGE: Rev = I4004CC::ULT; break;
  case I4004CC::UGT: Rev = I4004CC::ULE; break;
  case I4004CC::ULE: Rev = I4004CC::UGT; break;
  default: return true;
  }
  Cond[1].setImm(Rev);
  return false;
}

unsigned I4004InstrInfo::insertBranch(MachineBasicBlock &MBB,
                                      MachineBasicBlock *TBB,
                                      MachineBasicBlock *FBB,
                                      ArrayRef<MachineOperand> Cond,
                                      const DebugLoc &DL,
                                      int *BytesAdded) const {
  assert(TBB && "insertBranch must have a true destination");
  int Bytes = 0;
  unsigned Count = 0;

  if (Cond.empty()) {
    // Unconditional branch to TBB.
    assert(!FBB && "unconditional branch cannot have a false target");
    BuildMI(&MBB, DL, get(I4004::BR)).addMBB(TBB);
    Bytes += 2;
    ++Count;
  } else {
    // Conditional: Cond = [opcode, cc, lhs, rhs]. Emit the BRCC to TBB.
    assert(Cond.size() == 4 && "malformed i4004 branch condition");
    unsigned Op = Cond[0].getImm();
    BuildMI(&MBB, DL, get(Op))
        .add(Cond[2])            // lhs
        .add(Cond[3])            // rhs
        .addImm(Cond[1].getImm()) // cc
        .addMBB(TBB);
    Bytes += 2;
    ++Count;
    // A false target needs a trailing unconditional branch.
    if (FBB) {
      BuildMI(&MBB, DL, get(I4004::BR)).addMBB(FBB);
      Bytes += 2;
      ++Count;
    }
  }
  if (BytesAdded)
    *BytesAdded = Bytes;
  return Count;
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
    // Only remove branches we model (BR and the conditional BRCC pseudos);
    // stop at the first non-(modeled)-terminator.
    unsigned Op = I->getOpcode();
    if (Op != I4004::BR && Op != I4004::BRCCi8 && Op != I4004::BRCCi16)
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
