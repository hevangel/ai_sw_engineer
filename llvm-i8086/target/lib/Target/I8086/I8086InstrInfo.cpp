//===-- I8086InstrInfo.cpp - I8086 Instruction Information -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086InstrInfo.h"
#include "I8086.h"
#include "I8086Subtarget.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineMemOperand.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"

using namespace llvm;

#define GET_INSTRINFO_CTOR_DTOR
#include "I8086GenInstrInfo.inc"

void I8086InstrInfo::anchor() {}

I8086InstrInfo::I8086InstrInfo(const I8086Subtarget &STI)
    : I8086GenInstrInfo(STI, RI, I8086::ADJCALLSTACKDOWN,
                        I8086::ADJCALLSTACKUP),
      RI() {}

void I8086InstrInfo::copyPhysReg(MachineBasicBlock &MBB,
                                 MachineBasicBlock::iterator MI,
                                 const DebugLoc &DL, Register DestReg,
                                 Register SrcReg, bool KillSrc,
                                 bool RenamableDest, bool RenamableSrc) const {
  unsigned Opc;
  if (I8086::GR16RegClass.contains(DestReg, SrcReg))
    Opc = I8086::P_MOV16rr;
  else if (I8086::GR8RegClass.contains(DestReg, SrcReg))
    Opc = I8086::P_MOV8rr;
  else
    llvm_unreachable("cannot copy this register class");
  BuildMI(MBB, MI, DL, get(Opc), DestReg)
      .addReg(SrcReg, getKillRegState(KillSrc));
}

static MachineMemOperand *frameMMO(MachineFunction &MF, int FI,
                                   MachineMemOperand::Flags Flags,
                                   unsigned Size) {
  MachineFrameInfo &MFI = MF.getFrameInfo();
  return MF.getMachineMemOperand(MachinePointerInfo::getFixedStack(MF, FI),
                                 Flags, Size, MFI.getObjectAlign(FI));
}

// Spills are [bp + disp]: the frame index is the base of the memory operand
// and eliminateFrameIndex turns it into BP.
void I8086InstrInfo::storeRegToStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register SrcReg,
    bool isKill, int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    MachineInstr::MIFlag Flags) const {
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  bool Wide = I8086::GR16RegClass.hasSubClassEq(RC) ||
              RC == &I8086::GR16_ABCDRegClass || RC == &I8086::GR16_BASERegClass ||
              RC == &I8086::GR16_INDEXRegClass || RC == &I8086::GR16_PTRRegClass;
  MachineFunction &MF = *MBB.getParent();
  BuildMI(MBB, MI, DL, get(Wide ? I8086::P_MOV16mr : I8086::P_MOV8mr))
      .addFrameIndex(FrameIndex)
      .addReg(0)
      .addImm(0)
      .addReg(0)
      .addReg(SrcReg, getKillRegState(isKill))
      .addMemOperand(frameMMO(MF, FrameIndex, MachineMemOperand::MOStore,
                              Wide ? 2 : 1));
}

void I8086InstrInfo::loadRegFromStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register DestReg,
    int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    unsigned SubReg, MachineInstr::MIFlag Flags) const {
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  bool Wide = I8086::GR16RegClass.hasSubClassEq(RC) ||
              RC == &I8086::GR16_ABCDRegClass || RC == &I8086::GR16_BASERegClass ||
              RC == &I8086::GR16_INDEXRegClass || RC == &I8086::GR16_PTRRegClass;
  MachineFunction &MF = *MBB.getParent();
  BuildMI(MBB, MI, DL, get(Wide ? I8086::P_MOV16rm : I8086::P_MOV8rm), DestReg)
      .addFrameIndex(FrameIndex)
      .addReg(0)
      .addImm(0)
      .addReg(0)
      .addMemOperand(frameMMO(MF, FrameIndex, MachineMemOperand::MOLoad,
                              Wide ? 2 : 1));
}

bool I8086InstrInfo::analyzeBranch(MachineBasicBlock &MBB,
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
    if (Op == I8086::P_JMP) {
      if (Cond.empty())
        TBB = T->getOperand(0).getMBB();
      else
        FBB = T->getOperand(0).getMBB();
      continue;
    }
    if (Op == I8086::P_JCC) {
      if (!Cond.empty() || TBB)
        return true; // more than one conditional branch
      TBB = T->getOperand(0).getMBB();
      Cond.push_back(MachineOperand::CreateImm(T->getOperand(1).getImm()));
      continue;
    }
    return true;
  }
  return false;
}

bool I8086InstrInfo::reverseBranchCondition(
    SmallVectorImpl<MachineOperand> &Cond) const {
  if (Cond.size() != 1)
    return true;
  Cond[0].setImm(I8086CC::invert(Cond[0].getImm()));
  return false;
}

unsigned I8086InstrInfo::insertBranch(MachineBasicBlock &MBB,
                                      MachineBasicBlock *TBB,
                                      MachineBasicBlock *FBB,
                                      ArrayRef<MachineOperand> Cond,
                                      const DebugLoc &DL,
                                      int *BytesAdded) const {
  unsigned Count = 0;
  if (Cond.empty()) {
    BuildMI(&MBB, DL, get(I8086::P_JMP)).addMBB(TBB);
    Count = 1;
  } else {
    BuildMI(&MBB, DL, get(I8086::P_JCC)).addMBB(TBB).addImm(Cond[0].getImm());
    Count = 1;
    if (FBB) {
      BuildMI(&MBB, DL, get(I8086::P_JMP)).addMBB(FBB);
      ++Count;
    }
  }
  if (BytesAdded)
    *BytesAdded = 3 * Count; // sizes are not known before the assembler relaxes
  return Count;
}

unsigned I8086InstrInfo::removeBranch(MachineBasicBlock &MBB,
                                      int *BytesRemoved) const {
  unsigned Count = 0;
  MachineBasicBlock::iterator I = MBB.end();
  while (I != MBB.begin()) {
    --I;
    if (I->isDebugInstr())
      continue;
    if (I->getOpcode() != I8086::P_JMP && I->getOpcode() != I8086::P_JCC)
      break;
    I->eraseFromParent();
    I = MBB.end();
    ++Count;
  }
  if (BytesRemoved)
    *BytesRemoved = 3 * Count;
  return Count;
}
