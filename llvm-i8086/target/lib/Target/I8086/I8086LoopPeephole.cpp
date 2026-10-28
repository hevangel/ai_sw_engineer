//===-- I8086LoopPeephole.cpp - loop and jcxz -----------------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Before emission, a block that ends in
//
//     dec cx ; jne L        becomes    loop L
//     test cx, cx ; je L    becomes    jcxz L
//
// when the flags the compare leaves are not used afterwards (loop and jcxz do
// not touch them). `loop` and `jcxz` reach only 127 bytes and the assembler
// does not relax them (spec/codegen.md), so the rewrite is made only in a
// function that cannot be longer than that: at most MaxInstrs instructions of
// at most 7 bytes each, so any branch inside it is in range.
//
//===----------------------------------------------------------------------===//

#include "I8086.h"
#include "I8086InstrInfo.h"
#include "I8086Subtarget.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-loop"

namespace {

constexpr unsigned MaxInstrs = 17; // 17 * 7 bytes = 119 < 127

class I8086LoopPeephole : public MachineFunctionPass {
public:
  static char ID;
  I8086LoopPeephole() : MachineFunctionPass(ID) {}

  StringRef getPassName() const override { return "I8086 loop and jcxz"; }

  // Does `MI` leave ZF as a test of its destination would, and CF/OF as the
  // branch (E/NE only) does not care about?
  static bool setsZeroFlagOfDestination(const MachineInstr &MI, Register R) {
    switch (MI.getOpcode()) {
    case I8086::P_INC16r: case I8086::P_DEC16r: case I8086::P_NEG16r:
    case I8086::P_ADD16rr: case I8086::P_ADD16ri: case I8086::P_ADD16rm:
    case I8086::P_SUB16rr: case I8086::P_SUB16ri: case I8086::P_SUB16rm:
    case I8086::P_AND16rr: case I8086::P_AND16ri: case I8086::P_AND16rm:
    case I8086::P_OR16rr:  case I8086::P_OR16ri:  case I8086::P_OR16rm:
    case I8086::P_XOR16rr: case I8086::P_XOR16ri: case I8086::P_XOR16rm:
      return MI.getOperand(0).isReg() && MI.getOperand(0).getReg() == R;
    default:
      return false;
    }
  }

  // `dec r ; test r, r ; je/jne L`: the test repeats what the dec left in ZF.
  bool dropRedundantTests(MachineFunction &MF) {
    bool Changed = false;
    for (MachineBasicBlock &MBB : MF) {
      if (MBB.size() < 3)
        continue;
      MachineInstr &Br = MBB.back();
      if (Br.getOpcode() != I8086::P_JCC)
        continue;
      int CC = Br.getOperand(1).getImm();
      if (CC != I8086CC::E && CC != I8086CC::NE)
        continue;
      auto Test = std::prev(MBB.end());
      --Test;
      if (Test->getOpcode() != I8086::P_TEST16rr ||
          Test->getOperand(0).getReg() != Test->getOperand(1).getReg() ||
          Test == MBB.begin())
        continue;
      Register R = Test->getOperand(0).getReg();
      auto Prev = std::prev(Test);
      if (!setsZeroFlagOfDestination(*Prev, R))
        continue;
      bool FlagsLive = false;
      for (MachineBasicBlock *S : MBB.successors())
        FlagsLive |= S->isLiveIn(I8086::FLAGS);
      if (FlagsLive)
        continue;
      Test->eraseFromParent();
      Changed = true;
    }
    return Changed;
  }

  bool runOnMachineFunction(MachineFunction &MF) override {
    bool Changed = dropRedundantTests(MF);
    unsigned Count = 0;
    for (MachineBasicBlock &MBB : MF)
      for (MachineInstr &MI : MBB)
        if (!MI.isDebugInstr())
          ++Count;
    if (Count > MaxInstrs)
      return Changed;
    const TargetInstrInfo &TII = *MF.getSubtarget().getInstrInfo();

    for (MachineBasicBlock &MBB : MF) {
      if (MBB.empty())
        continue;
      MachineInstr &Br = MBB.back();
      if (Br.getOpcode() != I8086::P_JCC)
        continue;
      // The flags must not be live into a successor.
      bool FlagsLive = false;
      for (MachineBasicBlock *S : MBB.successors())
        FlagsLive |= S->isLiveIn(I8086::FLAGS);
      if (FlagsLive)
        continue;
      // The instruction before the branch.
      auto Prev = std::prev(MBB.end());
      if (Prev == MBB.begin())
        continue;
      --Prev;
      while (Prev->isDebugInstr() && Prev != MBB.begin())
        --Prev;
      int CC = Br.getOperand(1).getImm();
      unsigned NewOpc = 0;
      if (Prev->getOpcode() == I8086::P_DEC16r && CC == I8086CC::NE &&
          Prev->getOperand(0).getReg() == I8086::CX)
        NewOpc = I8086::LOOP;
      else if (Prev->getOpcode() == I8086::P_TEST16rr && CC == I8086CC::E &&
               Prev->getOperand(0).getReg() == I8086::CX &&
               Prev->getOperand(1).getReg() == I8086::CX)
        NewOpc = I8086::JCXZ;
      if (!NewOpc)
        continue;
      BuildMI(MBB, Br, Br.getDebugLoc(), TII.get(NewOpc)).add(Br.getOperand(0));
      Br.eraseFromParent();
      Prev->eraseFromParent();
      Changed = true;
    }
    return Changed;
  }
};

} // namespace

char I8086LoopPeephole::ID = 0;

FunctionPass *llvm::createI8086LoopPeephole() {
  return new I8086LoopPeephole();
}
