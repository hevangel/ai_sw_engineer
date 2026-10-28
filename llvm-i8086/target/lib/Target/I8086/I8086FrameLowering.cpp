//===-- I8086FrameLowering.cpp - I8086 Frame Lowering ----------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086FrameLowering.h"
#include "I8086InstrInfo.h"
#include "I8086MachineFunctionInfo.h"
#include "I8086Subtarget.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"

using namespace llvm;

// Stack objects are 2-byte aligned (a word access to an odd address costs a
// bus cycle on the 8088 and the 8086 alike). The return address is not part
// of the local area: the first local is just below the entry stack pointer.
I8086FrameLowering::I8086FrameLowering(const I8086Subtarget &STI)
    : TargetFrameLowering(TargetFrameLowering::StackGrowsDown, Align(2), 0,
                          Align(2)) {}

bool I8086FrameLowering::hasFPImpl(const MachineFunction &MF) const {
  // Conservative until assignCalleeSavedSpillSlots has fixed the answer.
  int HasFP = MF.getInfo<I8086MachineFunctionInfo>()->HasFP;
  return HasFP < 0 || HasFP != 0;
}

bool I8086FrameLowering::hasReservedCallFrame(const MachineFunction &MF) const {
  // Stack arguments are pushed, so SP moves around every call.
  return false;
}

void I8086FrameLowering::determineCalleeSaves(MachineFunction &MF,
                                              BitVector &SavedRegs,
                                              RegScavenger *RS) const {
  TargetFrameLowering::determineCalleeSaves(MF, SavedRegs, RS);
  // An interrupt handler saves every register, whatever it or its callees use.
  if (isInterruptHandler(MF.getFunction()))
    for (MCRegister R : {I8086::AX, I8086::CX, I8086::DX, I8086::BX, I8086::SI,
                         I8086::DI})
      SavedRegs.set(R);
  // BP is the frame pointer; the prologue saves it itself.
  SavedRegs.reset(I8086::BP);
}

bool I8086FrameLowering::assignCalleeSavedSpillSlots(
    MachineFunction &MF, const TargetRegisterInfo *TRI,
    std::vector<CalleeSavedInfo> &CSI) const {
  MachineFrameInfo &MFI = MF.getFrameInfo();
  auto *FI = MF.getInfo<I8086MachineFunctionInfo>();

  // Stack objects exist (locals, spills, incoming stack arguments) or the
  // stack grows by a variable amount: address the frame through BP.
  bool HasFP = MFI.hasVarSizedObjects() || MFI.getNumObjects() > 0 ||
               isInterruptHandler(MF.getFunction());
  FI->HasFP = HasFP;

  // Offsets are from the stack pointer at entry; the return address is at 0.
  int Off = 0;
  if (HasFP) {
    Off -= 2;
    MFI.CreateFixedSpillStackObject(2, Off); // the saved BP
  }
  for (CalleeSavedInfo &CS : CSI) {
    Off -= 2;
    CS.setFrameIdx(MFI.CreateFixedSpillStackObject(2, Off));
  }
  FI->NumPushed = CSI.size();
  if (isInterruptHandler(MF.getFunction())) {
    // DS and ES are pushed after the registers (emitPrologue).
    for (int I = 0; I < 2; ++I) {
      Off -= 2;
      MFI.CreateFixedSpillStackObject(2, Off);
    }
    FI->NumPushed += 2;
  }
  return true;
}

bool I8086FrameLowering::spillCalleeSavedRegisters(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI,
    ArrayRef<CalleeSavedInfo> CSI, const TargetRegisterInfo *TRI) const {
  const TargetInstrInfo &TII = *MBB.getParent()->getSubtarget().getInstrInfo();
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  for (const CalleeSavedInfo &CS : CSI) {
    MCRegister Reg = CS.getReg();
    MBB.addLiveIn(Reg);
    BuildMI(MBB, MI, DL, TII.get(I8086::P_PUSH16r))
        .addReg(Reg, RegState::Kill)
        .setMIFlag(MachineInstr::FrameSetup);
  }
  return true;
}

bool I8086FrameLowering::restoreCalleeSavedRegisters(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI,
    MutableArrayRef<CalleeSavedInfo> CSI, const TargetRegisterInfo *TRI) const {
  const TargetInstrInfo &TII = *MBB.getParent()->getSubtarget().getInstrInfo();
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  for (const CalleeSavedInfo &CS : reverse(CSI))
    BuildMI(MBB, MI, DL, TII.get(I8086::P_POP16r), CS.getReg())
        .setMIFlag(MachineInstr::FrameDestroy);
  return true;
}

void I8086FrameLowering::emitPrologue(MachineFunction &MF,
                                      MachineBasicBlock &MBB) const {
  const I8086InstrInfo &TII = *MF.getSubtarget<I8086Subtarget>().getInstrInfo();
  auto *FI = MF.getInfo<I8086MachineFunctionInfo>();
  MachineBasicBlock::iterator MBBI = MBB.begin();
  DebugLoc DL;

  if (FI->HasFP) {
    // Before the callee-saved pushes spillCalleeSavedRegisters placed.
    BuildMI(MBB, MBBI, DL, TII.get(I8086::P_PUSH16r))
        .addReg(I8086::BP, RegState::Kill)
        .setMIFlag(MachineInstr::FrameSetup);
    BuildMI(MBB, MBBI, DL, TII.get(I8086::P_MOV16rr), I8086::BP)
        .addReg(I8086::SP)
        .setMIFlag(MachineInstr::FrameSetup);
  }

  // Past the pushes, allocate the locals.
  while (MBBI != MBB.end() && MBBI->getFlag(MachineInstr::FrameSetup))
    ++MBBI;
  if (isInterruptHandler(MF.getFunction())) {
    // The handler runs with the interrupted program's DS and ES: keep them and
    // use the code segment (the tiny model: CS = DS = ES = SS).
    for (MCRegister Seg : {I8086::DS, I8086::ES})
      BuildMI(MBB, MBBI, DL, TII.get(I8086::PUSH16s))
          .addReg(Seg)
          .setMIFlag(MachineInstr::FrameSetup);
    for (MCRegister Seg : {I8086::DS, I8086::ES}) {
      BuildMI(MBB, MBBI, DL, TII.get(I8086::PUSH16s))
          .addReg(I8086::CS)
          .setMIFlag(MachineInstr::FrameSetup);
      BuildMI(MBB, MBBI, DL, TII.get(I8086::POP16s))
          .addReg(Seg, RegState::Define)
          .setMIFlag(MachineInstr::FrameSetup);
    }
    BuildMI(MBB, MBBI, DL, TII.get(I8086::CLD))
        .setMIFlag(MachineInstr::FrameSetup);
  }
  int64_t Saved = (FI->HasFP ? 2 : 0) + 2 * int64_t(FI->NumPushed);
  int64_t Adjust = int64_t(MF.getFrameInfo().getStackSize()) - Saved;
  if (FI->HasFP && Adjust > 0)
    BuildMI(MBB, MBBI, DL, TII.get(I8086::P_SUB16ri), I8086::SP)
        .addReg(I8086::SP)
        .addImm(Adjust)
        .setMIFlag(MachineInstr::FrameSetup);
}

void I8086FrameLowering::emitEpilogue(MachineFunction &MF,
                                      MachineBasicBlock &MBB) const {
  const I8086InstrInfo &TII = *MF.getSubtarget<I8086Subtarget>().getInstrInfo();
  auto *FI = MF.getInfo<I8086MachineFunctionInfo>();
  if (!FI->HasFP)
    return;
  MachineBasicBlock::iterator Ret = MBB.getFirstTerminator();
  DebugLoc DL = Ret != MBB.end() ? Ret->getDebugLoc() : DebugLoc();

  // Before the callee-saved pops: SP just below the saved registers.
  MachineBasicBlock::iterator Pops = Ret;
  while (Pops != MBB.begin()) {
    MachineBasicBlock::iterator Prev = std::prev(Pops);
    if (Prev->isDebugInstr() || !Prev->getFlag(MachineInstr::FrameDestroy))
      break;
    Pops = Prev;
  }
  if (FI->NumPushed)
    BuildMI(MBB, Pops, DL, TII.get(I8086::P_LEA16r), I8086::SP)
        .addReg(I8086::BP)
        .addReg(0)
        .addImm(-2 * int64_t(FI->NumPushed))
        .addReg(0)
        .setMIFlag(MachineInstr::FrameDestroy);
  else
    BuildMI(MBB, Pops, DL, TII.get(I8086::P_MOV16rr), I8086::SP)
        .addReg(I8086::BP)
        .setMIFlag(MachineInstr::FrameDestroy);
  if (isInterruptHandler(MF.getFunction())) {
    // After the lea: ES and DS come off first (pushed last).
    for (MCRegister Seg : {I8086::ES, I8086::DS})
      BuildMI(MBB, Pops, DL, TII.get(I8086::POP16s))
          .addReg(Seg, RegState::Define)
          .setMIFlag(MachineInstr::FrameDestroy);
  }
  BuildMI(MBB, Ret, DL, TII.get(I8086::P_POP16r), I8086::BP)
      .setMIFlag(MachineInstr::FrameDestroy);
  if (isInterruptHandler(MF.getFunction()) && Ret != MBB.end() &&
      Ret->getOpcode() == I8086::P_RET) {
    BuildMI(MBB, Ret, DL, TII.get(I8086::IRET));
    Ret->eraseFromParent();
  }
}

MachineBasicBlock::iterator I8086FrameLowering::eliminateCallFramePseudoInstr(
    MachineFunction &MF, MachineBasicBlock &MBB,
    MachineBasicBlock::iterator I) const {
  const TargetInstrInfo &TII = *MF.getSubtarget().getInstrInfo();
  if (I->getOpcode() == I8086::ADJCALLSTACKUP) {
    // The caller removes the arguments it pushed (abi.md D3).
    int64_t Amount = I->getOperand(0).getImm();
    if (Amount)
      BuildMI(MBB, I, I->getDebugLoc(), TII.get(I8086::P_ADD16ri), I8086::SP)
          .addReg(I8086::SP)
          .addImm(Amount);
  }
  return MBB.erase(I);
}
