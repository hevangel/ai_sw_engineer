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

const uint32_t *
I4004RegisterInfo::getCallPreservedMask(const MachineFunction &MF,
                                        CallingConv::ID) const {
  return CSR_I4004_RegMask;
}

const uint32_t *I4004RegisterInfo::getLeafCallPreservedMask() {
  return CSR_I4004_Leaf_RegMask;
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

  // SPILL/RELOAD/LEA_FI carry the slot as a frame index. Per-function frames
  // (abi.md 4.4, brief decision 1): convert the FI to the WITHIN-FRAME char
  // offset. I4004ExpandPseudos emits the access against the external symbol
  // __frame_<fn> (RAM char base assigned per-function by the packer overlay,
  // bank 0). The char offset is 2*(objectOffset - minObjectOffset): the REAL
  // frame layout, so each object -- including a multi-byte alloca -- occupies
  // its true extent and distinct objects never alias. (Using FrameIndex*2 was
  // wrong: it ignored object SIZE, so a 68-byte Calculator alloca and the next
  // slot differed by 2 chars, and the memset that zeroes it overran the soft
  // stack.) The frame's TOTAL size is published separately as
  // __framesize_<fn> (AsmPrinter) so the packer reserves the full frame.
  unsigned Op = MI.getOpcode();
  if (Op == I4004::SPILL8 || Op == I4004::SPILL16 || Op == I4004::RELOAD8 ||
      Op == I4004::RELOAD16 || Op == I4004::LEA_FI) {
    const MachineFrameInfo &MFI = MF.getFrameInfo();
    // minimum object offset across the frame (StackGrowsUp => offsets >= 0, but
    // normalize defensively so the frame is 0-based).
    int MinOff = 0;
    bool First = true;
    for (int I = MFI.getObjectIndexBegin(), E = MFI.getObjectIndexEnd();
         I < E; ++I) {
      if (MFI.isDeadObjectIndex(I))
        continue;
      int O = MFI.getObjectOffset(I);
      if (First || O < MinOff) { MinOff = O; First = false; }
    }
    int CharOff = 2 * (Offset - MinOff);
    if (getenv("I4004_FRAME_DEBUG"))
      errs() << "FRAME " << MF.getName() << " op=" << Op
             << " FI=" << FrameIndex << " off=" << Offset
             << " size=" << MFI.getObjectSize(FrameIndex)
             << " -> char " << CharOff << " (framesize_chars="
             << (MFI.getStackSize() * 2) << ")\n";
    MI.getOperand(FIOperandNum).ChangeToImmediate(CharOff & 0xFF);
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
