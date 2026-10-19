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
  // 16-bit double-pair copy: copy each 8-bit sub-pair via COPYpr.
  if (I4004::DR16RegClass.contains(DestReg, SrcReg)) {
    Register DLo = RI.getSubReg(DestReg, I4004::sub_pair_lo);
    Register DHi = RI.getSubReg(DestReg, I4004::sub_pair_hi);
    Register SLo = RI.getSubReg(SrcReg, I4004::sub_pair_lo);
    Register SHi = RI.getSubReg(SrcReg, I4004::sub_pair_hi);
    BuildMI(MBB, MI, DL, get(I4004::COPYpr), DLo).addReg(SLo);
    BuildMI(MBB, MI, DL, get(I4004::COPYpr), DHi)
        .addReg(SHi, getKillRegState(KillSrc));
    return;
  }
  // Pair-to-pair copy: COPYpr, expanded later into two nibble moves.
  BuildMI(MBB, MI, DL, get(I4004::COPYpr), DestReg)
      .addReg(SrcReg, getKillRegState(KillSrc));
}

void I4004InstrInfo::storeRegToStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register SrcReg,
    bool isKill, int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    MachineInstr::MIFlag Flags) const {
  // Spills to 4002 RAM are a later milestone (abi.md sec.2.1 imaginary regs).
  // Emitting nothing would miscompile; report so the limitation is explicit.
  report_fatal_error("I4004: register spilling to 4002 RAM is not yet "
                     "implemented (milestone: imaginary-register frame)");
}

void I4004InstrInfo::loadRegFromStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register DestReg,
    int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    unsigned SubReg, MachineInstr::MIFlag Flags) const {
  report_fatal_error("I4004: register reloading from 4002 RAM is not yet "
                     "implemented (milestone: imaginary-register frame)");
}
