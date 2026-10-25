//===-- I8008RegisterInfo.cpp - I8008 Register Information ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008RegisterInfo.h"
#include "I8008.h"
#include "I8008Subtarget.h"
#include "llvm/ADT/BitVector.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/IR/Function.h"
#include "llvm/MC/MCContext.h"

using namespace llvm;

#define DEBUG_TYPE "i8008-reg-info"

#define GET_REGINFO_TARGET_DESC
#include "I8008GenRegisterInfo.inc"

I8008RegisterInfo::I8008RegisterInfo() : I8008GenRegisterInfo(I8008::A) {}

const MCPhysReg *
I8008RegisterInfo::getCalleeSavedRegs(const MachineFunction *MF) const {
  return CSR_I8008_SaveList;
}

const uint32_t *
I8008RegisterInfo::getCallPreservedMask(const MachineFunction &MF,
                                        CallingConv::ID) const {
  return CSR_I8008_RegMask;
}

const uint32_t *I8008RegisterInfo::leafMask() { return CSR_I8008_Leaf_RegMask; }
const uint32_t *I8008RegisterInfo::leafVoidMask() {
  return CSR_I8008_LeafVoid_RegMask;
}

BitVector I8008RegisterInfo::getReservedRegs(const MachineFunction &MF) const {
  BitVector Reserved(getNumRegs());
  // abi.md sec.2: A, H, L and the flags are expansion scratch.
  for (MCPhysReg R : {I8008::A, I8008::H, I8008::L, I8008::HL, I8008::F})
    Reserved.set(R);
  return Reserved;
}

const TargetRegisterClass *
I8008RegisterInfo::getPointerRegClass(unsigned Kind) const {
  return &I8008::GR16RegClass;
}

const char *I8008RegisterInfo::frameSymbol(const MachineFunction &MF) {
  std::string S = ("__frame_" + MF.getName()).str();
  char *P = static_cast<char *>(MF.getContext().allocate(S.size() + 1, 1));
  memcpy(P, S.c_str(), S.size() + 1);
  return P;
}

// A frame index operand is the base of a memabs pair (base, off). It becomes
// the external symbol __frame_<fn> with the object's offset in the frame plus
// off (abi.md sec.4). Offsets are the real object layout, normalised to start
// at 0, so multi-byte objects never alias.
bool I8008RegisterInfo::eliminateFrameIndex(MachineBasicBlock::iterator II,
                                            int SPAdj, unsigned FIOperandNum,
                                            RegScavenger *RS) const {
  MachineInstr &MI = *II;
  MachineFunction &MF = *MI.getParent()->getParent();
  const MachineFrameInfo &MFI = MF.getFrameInfo();
  int FI = MI.getOperand(FIOperandNum).getIndex();

  int MinOff = 0;
  bool First = true;
  for (int I = MFI.getObjectIndexBegin(), E = MFI.getObjectIndexEnd(); I < E;
       ++I) {
    if (MFI.isDeadObjectIndex(I))
      continue;
    int O = MFI.getObjectOffset(I);
    if (First || O < MinOff) {
      MinOff = O;
      First = false;
    }
  }
  int64_t Off = MFI.getObjectOffset(FI) - MinOff;
  assert(MI.getNumOperands() > FIOperandNum + 1 &&
         MI.getOperand(FIOperandNum + 1).isImm() &&
         "frame index must be the base of a memabs operand");
  Off += MI.getOperand(FIOperandNum + 1).getImm();
  MI.getOperand(FIOperandNum + 1).setImm(0);
  MachineOperand &MO = MI.getOperand(FIOperandNum);
  MO.ChangeToES(frameSymbol(MF));
  MO.setOffset(Off);
  return false;
}

Register I8008RegisterInfo::getFrameRegister(const MachineFunction &MF) const {
  return I8008::HL;
}
