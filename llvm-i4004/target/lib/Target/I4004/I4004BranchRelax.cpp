//===-- I4004BranchRelax.cpp - make conditional branches page-independent -===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The 4004 `JCN`/`ISZ` address field is 8 bits -- a branch can only reach a
// target in the CURRENT 256-byte ROM page. Any function larger than a page has
// intra-function conditional branches whose target is on another page and
// cannot be encoded, so no image links. This pass rewrites every conditional
// branch into a PAGE-INDEPENDENT form so a function may span pages freely (the
// packer then needs no `--pad-pages`):
//
//   JCN cc, Dest      =>   JCN !cc, L ; JUN Dest ; L:(fall-through)
//   ISZ Rn, Dest      =>   ISZ Rn, L ; JUN FT ; L: JUN Dest   (+ FT explicit)
//
// The near `JCN`/`ISZ` only skips a 3-byte `JUN` (same page, except exactly at
// a page boundary, which the packer handles / avoids), and `JUN` carries a full
// 12-bit address. This runs after I4004ExpandPseudos (when JCN/ISZ with MBB
// targets exist) and before the peephole. It is unconditional: every
// conditional branch is relaxed, trading ~2 bytes each for position
// independence -- correctness first.
//
//===----------------------------------------------------------------------===//

#include "I4004.h"
#include "I4004InstrInfo.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-branch-relax"

namespace {
class I4004BranchRelax : public MachineFunctionPass {
public:
  static char ID;
  I4004BranchRelax() : MachineFunctionPass(ID) {}
  bool runOnMachineFunction(MachineFunction &MF) override;
  StringRef getPassName() const override {
    return "I4004 far-branch relaxation";
  }

private:
  const TargetInstrInfo *TII = nullptr;
};
} // namespace

char I4004BranchRelax::ID = 0;

bool I4004BranchRelax::runOnMachineFunction(MachineFunction &MF) {
  TII = MF.getSubtarget().getInstrInfo();
  bool Changed = false;

  // Collect the conditional branches first (we mutate the function as we go).
  SmallVector<MachineInstr *, 32> Cond;
  for (MachineBasicBlock &MBB : MF)
    for (MachineInstr &MI : MBB) {
      unsigned Op = MI.getOpcode();
      if (Op == I4004::JCN || Op == I4004::ISZ)
        if (MI.getOperand(MI.getNumOperands() - 1).isMBB())
          Cond.push_back(&MI);
    }

  for (MachineInstr *MI : Cond) {
    MachineBasicBlock &MBB = *MI->getParent();
    DebugLoc DL = MI->getDebugLoc();
    unsigned Op = MI->getOpcode();
    MachineBasicBlock *Dest = MI->getOperand(MI->getNumOperands() - 1).getMBB();

    // Split MBB right after the branch so the inserted JUN(s) + the skip label
    // live in their own blocks and the CFG stays consistent. Everything after
    // the branch (should be nothing for a true terminator, or the layout
    // fall-through) moves to ContBB.
    MachineBasicBlock *ContBB = MF.CreateMachineBasicBlock(MBB.getBasicBlock());
    MF.insert(std::next(MBB.getIterator()), ContBB);
    // Move any instructions after the branch into ContBB.
    ContBB->splice(ContBB->begin(), &MBB, std::next(MI->getIterator()),
                   MBB.end());
    ContBB->transferSuccessorsAndUpdatePHIs(&MBB);

    MachineBasicBlock::iterator InsPt = MI->getIterator();

    if (Op == I4004::JCN) {
      int Cc = MI->getOperand(0).getImm();
      int Inv = Cc ^ 0x8; // toggle the JCN "invert" bit (isa.md sec.4)
      // JCN !cc, ContBB ; JUN Dest   (ContBB is the physical fall-through)
      BuildMI(MBB, InsPt, DL, TII->get(I4004::JCN)).addImm(Inv).addMBB(ContBB);
      BuildMI(MBB, InsPt, DL, TII->get(I4004::JUN)).addMBB(Dest);
    } else { // ISZ Rn, Dest
      Register Rn = MI->getOperand(0).getReg();
      // ISZ Rn, Taken ; JUN ContBB ; Taken: JUN Dest
      MachineBasicBlock *Taken = MF.CreateMachineBasicBlock(MBB.getBasicBlock());
      MF.insert(ContBB->getIterator(), Taken); // place between MBB and ContBB
      BuildMI(MBB, InsPt, DL, TII->get(I4004::ISZ)).addReg(Rn).addMBB(Taken);
      BuildMI(MBB, InsPt, DL, TII->get(I4004::JUN)).addMBB(ContBB);
      BuildMI(*Taken, Taken->end(), DL, TII->get(I4004::JUN)).addMBB(Dest);
      MBB.addSuccessor(Taken);
      Taken->addSuccessor(Dest);
    }
    MI->eraseFromParent();
    MBB.addSuccessor(ContBB);
    MBB.addSuccessor(Dest);
    // Force labels so the AsmPrinter emits ContBB/Taken even as fall-throughs.
    ContBB->setLabelMustBeEmitted();
    Dest->setLabelMustBeEmitted();
    Changed = true;
  }
  return Changed;
}

INITIALIZE_PASS(I4004BranchRelax, DEBUG_TYPE, "I4004 far-branch relaxation",
                false, false)

namespace llvm {
FunctionPass *createI4004BranchRelaxPass() { return new I4004BranchRelax(); }
void initializeI4004BranchRelaxPass(PassRegistry &);
} // namespace llvm
