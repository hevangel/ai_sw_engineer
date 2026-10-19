//===-- I4004RegisterInfo.cpp - I4004 Register Information ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004RegisterInfo.h"
#include "I4004.h"
#include "I4004Subtarget.h"
#include "llvm/ADT/BitVector.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/TargetFrameLowering.h"
#include "llvm/IR/Function.h"
#include "llvm/Support/raw_ostream.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-reg-info"

#define GET_REGINFO_TARGET_DESC
#include "I4004GenRegisterInfo.inc"

// The "return address" slot concept is the hardware stack; we pass P7 as the
// frame register placeholder (software stack pointer, abi.md sec.4.4).
I4004RegisterInfo::I4004RegisterInfo() : I4004GenRegisterInfo(I4004::P7) {}

const MCPhysReg *
I4004RegisterInfo::getCalleeSavedRegs(const MachineFunction *MF) const {
  // abi.md sec.4.3: P6 is the only callee-saved pair. P4:P5 (double-pair D2)
  // is a caller-saved i16 argument/return pair (a `{ i16, ptr }` return places
  // its second word in D2), so it is clobbered by a call, not preserved.
  static const MCPhysReg CSR[] = {I4004::P6, 0};
  return CSR;
}

BitVector I4004RegisterInfo::getReservedRegs(const MachineFunction &MF) const {
  BitVector Reserved(getNumRegs());

  // Fixed hardware resources.
  Reserved.set(I4004::ACC);
  Reserved.set(I4004::CY);

  // P0 (R0:R1): FIN/JIN scratch pointer. P7 (R14:R15): software stack pointer.
  Reserved.set(I4004::P0);
  Reserved.set(I4004::R0);
  Reserved.set(I4004::R1);
  Reserved.set(I4004::P7);
  Reserved.set(I4004::R14);
  Reserved.set(I4004::R15);

  return Reserved;
}

bool I4004RegisterInfo::eliminateFrameIndex(MachineBasicBlock::iterator II,
                                            int SPAdj, unsigned FIOperandNum,
                                            RegScavenger *RS) const {
  MachineInstr &MI = *II;
  int FrameIndex = MI.getOperand(FIOperandNum).getIndex();
  MachineFunction &MF = *MI.getParent()->getParent();
  int Offset = MF.getFrameInfo().getObjectOffset(FrameIndex);

  // SPILL/RELOAD/LEA_FI carry the slot as a frame index in a dedicated
  // operand. Per-function frames (abi.md 4.4, brief decision 1): instead of a
  // single fixed shared RAM region (the old 0x30/reg3 base, which every
  // function reused so a callee clobbered its caller's spills), convert the FI
  // to the WITHIN-FRAME char offset. I4004ExpandPseudos then emits the access
  // against the external symbol __frame_<fn> (RAM char base, assigned
  // per-function by the packer's call-graph overlay, bank 0). We use the frame
  // INDEX (not getObjectOffset) as the slot number so each distinct stack
  // object -- including a callee-saved-register spill slot -- gets its own
  // chars; StackGrowsUp can map two distinct objects to offset 0, which would
  // alias a CSR save with a spill in the same function.
  unsigned Op = MI.getOpcode();
  if (Op == I4004::SPILL8 || Op == I4004::SPILL16 || Op == I4004::RELOAD8 ||
      Op == I4004::RELOAD16 || Op == I4004::LEA_FI) {
    int Slot = FrameIndex < 0 ? -FrameIndex : FrameIndex; // index -> slot
    (void)Offset;
    if (getenv("I4004_FRAME_DEBUG"))
      errs() << "FRAME " << MF.getName() << " op=" << Op
             << " FI=" << FrameIndex << " off=" << Offset
             << " -> char " << (Slot * 2) << "\n";
    MI.getOperand(FIOperandNum).ChangeToImmediate((Slot * 2) & 0xFF);
    return false;
  }

  // Generic case (none emitted by the backbone yet): point the FI operand at
  // the soft-SP register with the object offset.
  MI.getOperand(FIOperandNum).ChangeToRegister(I4004::P7, false);
  if (MI.getNumOperands() > FIOperandNum + 1 &&
      MI.getOperand(FIOperandNum + 1).isImm())
    MI.getOperand(FIOperandNum + 1).setImm(Offset);
  return false;
}

Register I4004RegisterInfo::getFrameRegister(const MachineFunction &MF) const {
  return I4004::P7;
}

const TargetRegisterClass *
I4004RegisterInfo::getPointerRegClass(unsigned Kind) const {
  // Pointers are 16-bit handles in a double-pair (abi.md sec.1.3).
  return &I4004::DR16RegClass;
}
