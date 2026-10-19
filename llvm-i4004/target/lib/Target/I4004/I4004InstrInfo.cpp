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
