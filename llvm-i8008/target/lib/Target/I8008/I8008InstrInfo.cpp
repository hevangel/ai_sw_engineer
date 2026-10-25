//===-- I8008InstrInfo.cpp - I8008 Instruction Information ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008InstrInfo.h"
#include "I8008.h"
#include "I8008ISelLowering.h"
#include "I8008Subtarget.h"
#include "MCTargetDesc/I8008OpTables.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"

using namespace llvm;

#define GET_INSTRINFO_CTOR_DTOR
#include "I8008GenInstrInfo.inc"

void I8008InstrInfo::anchor() {}

I8008InstrInfo::I8008InstrInfo(const I8008Subtarget &STI)
    : I8008GenInstrInfo(STI, RI, I8008::ADJCALLSTACKDOWN, I8008::ADJCALLSTACKUP),
      RI() {}

unsigned I8008InstrInfo::regField(Register R) {
  switch (R.id()) {
  case I8008::A: return 0;
  case I8008::B: return 1;
  case I8008::C: return 2;
  case I8008::D: return 3;
  case I8008::E: return 4;
  case I8008::H: return 5;
  case I8008::L: return 6;
  default:
    llvm_unreachable("not an 8-bit register");
  }
}

// Real moves directly (copyPhysReg can run after the pseudo expander).
void I8008InstrInfo::copyPhysReg(MachineBasicBlock &MBB,
                                 MachineBasicBlock::iterator MI,
                                 const DebugLoc &DL, Register DestReg,
                                 Register SrcReg, bool KillSrc,
                                 bool RenamableDest, bool RenamableSrc) const {
  auto Move = [&](Register D, Register S) {
    if (D == S)
      return;
    BuildMI(MBB, MI, DL, get(I8008Op::MOV[regField(D)][regField(S)]));
  };
  if (I8008::GR16RegClass.contains(DestReg) || DestReg == I8008::HL) {
    Move(RI.getSubReg(DestReg, I8008::sub_lo), RI.getSubReg(SrcReg, I8008::sub_lo));
    Move(RI.getSubReg(DestReg, I8008::sub_hi), RI.getSubReg(SrcReg, I8008::sub_hi));
    return;
  }
  Move(DestReg, SrcReg);
}

// Spills go to the static frame (abi.md sec.4): STA/LDA with the frame index
// as the memabs base.
void I8008InstrInfo::storeRegToStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register SrcReg,
    bool isKill, int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    MachineInstr::MIFlag Flags) const {
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  unsigned Opc = I8008::GR16RegClass.hasSubClassEq(RC) ? I8008::STA16
                                                       : I8008::STA8;
  BuildMI(MBB, MI, DL, get(Opc))
      .addReg(SrcReg, getKillRegState(isKill))
      .addFrameIndex(FrameIndex)
      .addImm(0);
}

void I8008InstrInfo::loadRegFromStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register DestReg,
    int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    unsigned SubReg, MachineInstr::MIFlag Flags) const {
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  unsigned Opc = I8008::GR16RegClass.hasSubClassEq(RC) ? I8008::LDA16
                                                       : I8008::LDA8;
  BuildMI(MBB, MI, DL, get(Opc), DestReg).addFrameIndex(FrameIndex).addImm(0);
}

static bool isCondBr(unsigned Op) {
  return Op == I8008::BRCC8rr || Op == I8008::BRCC8ri ||
         Op == I8008::BRCC16rr || Op == I8008::BRCC16ri;
}

bool I8008InstrInfo::analyzeBranch(MachineBasicBlock &MBB,
                                   MachineBasicBlock *&TBB,
                                   MachineBasicBlock *&FBB,
                                   SmallVectorImpl<MachineOperand> &Cond,
                                   bool AllowModify) const {
  TBB = FBB = nullptr;
  Cond.clear();
  MachineBasicBlock::iterator I = MBB.getLastNonDebugInstr();
  if (I == MBB.end() || !isUnpredicatedTerminator(*I))
    return false;
  MachineBasicBlock::iterator FirstTerm = I;
  while (FirstTerm != MBB.begin()) {
    auto Prev = std::prev(FirstTerm);
    if (!isUnpredicatedTerminator(*Prev))
      break;
    FirstTerm = Prev;
  }
  for (auto T = FirstTerm; T != MBB.end(); ++T) {
    if (T->isDebugInstr())
      continue;
    unsigned Op = T->getOpcode();
    if (Op == I8008::BR) {
      if (Cond.empty())
        TBB = T->getOperand(0).getMBB();
      else
        FBB = T->getOperand(0).getMBB();
      continue;
    }
    if (isCondBr(Op)) {
      if (!Cond.empty() || TBB)
        return true;
      TBB = T->getOperand(3).getMBB();
      Cond.push_back(MachineOperand::CreateImm(Op));
      Cond.push_back(MachineOperand::CreateImm(T->getOperand(2).getImm()));
      Cond.push_back(T->getOperand(0));
      Cond.push_back(T->getOperand(1));
      continue;
    }
    return true;
  }
  return false;
}

bool I8008InstrInfo::reverseBranchCondition(
    SmallVectorImpl<MachineOperand> &Cond) const {
  if (Cond.size() != 4)
    return true;
  Cond[1].setImm(I8008CC::invert(Cond[1].getImm()));
  return false;
}

unsigned I8008InstrInfo::insertBranch(MachineBasicBlock &MBB,
                                      MachineBasicBlock *TBB,
                                      MachineBasicBlock *FBB,
                                      ArrayRef<MachineOperand> Cond,
                                      const DebugLoc &DL,
                                      int *BytesAdded) const {
  unsigned Count = 0;
  if (Cond.empty()) {
    BuildMI(&MBB, DL, get(I8008::BR)).addMBB(TBB);
    Count = 1;
  } else {
    BuildMI(&MBB, DL, get(Cond[0].getImm()))
        .add(Cond[2])
        .add(Cond[3])
        .addImm(Cond[1].getImm())
        .addMBB(TBB);
    Count = 1;
    if (FBB) {
      BuildMI(&MBB, DL, get(I8008::BR)).addMBB(FBB);
      ++Count;
    }
  }
  if (BytesAdded)
    *BytesAdded = 3 * Count;
  return Count;
}

unsigned I8008InstrInfo::removeBranch(MachineBasicBlock &MBB,
                                      int *BytesRemoved) const {
  unsigned Count = 0;
  MachineBasicBlock::iterator I = MBB.end();
  while (I != MBB.begin()) {
    --I;
    if (I->isDebugInstr())
      continue;
    if (I->getOpcode() != I8008::BR && !isCondBr(I->getOpcode()))
      break;
    I->eraseFromParent();
    I = MBB.end();
    ++Count;
  }
  if (BytesRemoved)
    *BytesRemoved = 3 * Count;
  return Count;
}

unsigned I8008InstrInfo::getInstSizeInBytes(const MachineInstr &MI) const {
  if (MI.isMetaInstruction())
    return 0;
  return get(MI.getOpcode()).getSize();
}
