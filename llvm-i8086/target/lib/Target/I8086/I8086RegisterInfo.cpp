//===-- I8086RegisterInfo.cpp - I8086 Register Information ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086RegisterInfo.h"
#include "I8086.h"
#include "I8086MachineFunctionInfo.h"
#include "I8086Subtarget.h"
#include "llvm/ADT/BitVector.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/LiveIntervals.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-reg-info"

#define GET_REGINFO_TARGET_DESC
#include "I8086GenRegisterInfo.inc"

I8086RegisterInfo::I8086RegisterInfo() : I8086GenRegisterInfo(I8086::SP) {}

const MCPhysReg *
I8086RegisterInfo::getCalleeSavedRegs(const MachineFunction *MF) const {
  if (MF && isInterruptHandler(MF->getFunction()))
    return CSR_I8086_Intr_SaveList;
  return CSR_I8086_SaveList;
}

const uint32_t *
I8086RegisterInfo::getCallPreservedMask(const MachineFunction &MF,
                                        CallingConv::ID) const {
  return CSR_I8086_RegMask;
}

BitVector I8086RegisterInfo::getReservedRegs(const MachineFunction &MF) const {
  BitVector Reserved(getNumRegs());
  // SP and BP are the stack and frame pointers (abi.md D2, D4); the segment
  // registers are fixed by the tiny model.
  for (MCPhysReg R : {I8086::SP, I8086::BP, I8086::FLAGS, I8086::CS, I8086::DS,
                      I8086::ES, I8086::SS})
    Reserved.set(R);
  return Reserved;
}

const TargetRegisterClass *
I8086RegisterInfo::getPointerRegClass(unsigned Kind) const {
  return &I8086::GR16RegClass;
}

// A frame index is the base of a memory operand (base, index, disp, seg). The
// frame is addressed through BP: an object at offset O from the stack pointer
// at entry (the return address is at 0) is at BP + O + 2, because BP is the
// stack pointer after `push bp`.
bool I8086RegisterInfo::eliminateFrameIndex(MachineBasicBlock::iterator II,
                                            int SPAdj, unsigned FIOperandNum,
                                            RegScavenger *RS) const {
  MachineInstr &MI = *II;
  MachineFunction &MF = *MI.getParent()->getParent();
  const MachineFrameInfo &MFI = MF.getFrameInfo();
  assert(MF.getInfo<I8086MachineFunctionInfo>()->HasFP == 1 &&
         "a frame index needs the frame pointer");
  int FI = MI.getOperand(FIOperandNum).getIndex();
  MachineOperand &Disp = MI.getOperand(FIOperandNum + 2);
  assert(Disp.isImm() && "frame index with a symbolic displacement");
  Disp.setImm(Disp.getImm() + MFI.getObjectOffset(FI) + 2);
  MI.getOperand(FIOperandNum).ChangeToRegister(I8086::BP, /*isDef=*/false);
  return false;
}

Register I8086RegisterInfo::getFrameRegister(const MachineFunction &MF) const {
  return I8086::BP;
}

bool I8086RegisterInfo::shouldCoalesce(
    MachineInstr *MI, const TargetRegisterClass *SrcRC, unsigned SubReg,
    const TargetRegisterClass *DstRC, unsigned DstSubReg,
    const TargetRegisterClass *NewRC, LiveIntervals &LIS) const {
  bool Narrow = NewRC == &I8086::GR16_BASERegClass ||
                NewRC == &I8086::GR16_INDEXRegClass ||
                NewRC == &I8086::GR16_PTRRegClass;
  // Joining two values already in that class is fine.
  return !Narrow || (SrcRC == NewRC && DstRC == NewRC);
}
