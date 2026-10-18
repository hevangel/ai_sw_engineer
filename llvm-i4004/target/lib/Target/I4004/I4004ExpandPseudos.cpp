//===-- I4004ExpandPseudos.cpp - Expand I4004 pseudo-instructions --------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Post-RA pass that expands the ACC-centric CodeGen pseudos (I4004InstrCodeGen
// .td) into real MCS-4 instruction sequences. The 4004 has a single 4-bit
// accumulator and no register-to-register ALU, so an 8-bit pair operation
// becomes a sequence of LD/ADD/SUB/XCH over the two nibbles via ACC, with CY
// carrying between them (spec/isa.md, abi.md sec.2).
//
//===----------------------------------------------------------------------===//

#include "I4004.h"
#include "I4004InstrInfo.h"
#include "I4004Subtarget.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-expand-pseudos"
#define PASS_NAME "I4004 pseudo-instruction expansion"

namespace {

class I4004ExpandPseudos : public MachineFunctionPass {
public:
  static char ID;
  I4004ExpandPseudos() : MachineFunctionPass(ID) {}

  bool runOnMachineFunction(MachineFunction &MF) override;

  StringRef getPassName() const override { return PASS_NAME; }

private:
  const I4004InstrInfo *TII = nullptr;
  const TargetRegisterInfo *TRI = nullptr;

  // Return the hi (sub_hi) and lo (sub_lo) nibble physical registers of a pair.
  std::pair<Register, Register> nibbles(Register Pair) const {
    Register Hi = TRI->getSubReg(Pair, I4004::sub_hi);
    Register Lo = TRI->getSubReg(Pair, I4004::sub_lo);
    return {Hi, Lo};
  }

  bool expandMI(MachineBasicBlock &MBB, MachineInstr &MI);
  void emitAddSub(MachineBasicBlock &MBB, MachineInstr &MI, unsigned AluOpc);
  void emitCopy(MachineBasicBlock &MBB, MachineInstr &MI);
};

} // namespace

char I4004ExpandPseudos::ID = 0;

INITIALIZE_PASS(I4004ExpandPseudos, DEBUG_TYPE, PASS_NAME, false, false)

// Move the nibble in index register Src into index register Dst via ACC:
// LD Src ; XCH Dst.
static void moveNibble(MachineBasicBlock &MBB, MachineInstr &MI,
                       const I4004InstrInfo *TII, Register Dst, Register Src) {
  const DebugLoc &DL = MI.getDebugLoc();
  BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Src);
  BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(Dst, RegState::Define);
}

void I4004ExpandPseudos::emitCopy(MachineBasicBlock &MBB, MachineInstr &MI) {
  Register Dst = MI.getOperand(0).getReg();
  Register Src = MI.getOperand(1).getReg();
  auto [DHi, DLo] = nibbles(Dst);
  auto [SHi, SLo] = nibbles(Src);
  moveNibble(MBB, MI, TII, DHi, SHi);
  moveNibble(MBB, MI, TII, DLo, SLo);
}

// dst = lhs (ALU) rhs over two nibbles, carry propagating low->high.
//   ADD: CLC ; (LD lhs_i ; ADD rhs_i ; XCH dst_i) low then high -- CY chains.
//   SUB: the 4004 SUB computes {CY,ACC} <- ACC + ~rhs + ~CY (isa.md sec.7), so
//        the carry-out's polarity is inverted relative to the borrow the next
//        nibble needs. Start CLC, and insert CMC between nibbles to flip CY so
//        each nibble's ~CY injects the correct borrow. Verified 8/8 on the real
//        external/chips CPU (scripts/exec_fn.py; the earlier STC-once form was
//        wrong on every case -- an execution-test-caught backend bug).
void I4004ExpandPseudos::emitAddSub(MachineBasicBlock &MBB, MachineInstr &MI,
                                    unsigned AluOpc) {
  const DebugLoc &DL = MI.getDebugLoc();
  Register Dst = MI.getOperand(0).getReg();
  Register Lhs = MI.getOperand(1).getReg();
  Register Rhs = MI.getOperand(2).getReg();
  auto [DHi, DLo] = nibbles(Dst);
  auto [LHi, LLo] = nibbles(Lhs);
  auto [RHi, RLo] = nibbles(Rhs);
  const bool IsSub = (AluOpc == I4004::SUB);

  BuildMI(MBB, MI, DL, TII->get(I4004::CLC));

  // Low nibble.
  BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(LLo);
  BuildMI(MBB, MI, DL, TII->get(AluOpc)).addReg(RLo);
  BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DLo, RegState::Define);

  // Flip the borrow polarity for the subtract chain before the high nibble.
  if (IsSub)
    BuildMI(MBB, MI, DL, TII->get(I4004::CMC));

  // High nibble (carry/borrow from the low nibble is live in CY).
  BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(LHi);
  BuildMI(MBB, MI, DL, TII->get(AluOpc)).addReg(RHi);
  BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DHi, RegState::Define);
}

bool I4004ExpandPseudos::expandMI(MachineBasicBlock &MBB, MachineInstr &MI) {
  const DebugLoc &DL = MI.getDebugLoc();
  switch (MI.getOpcode()) {
  default:
    return false;

  case I4004::RETpseudo:
    // Return on the hardware stack: BBL 0.
    BuildMI(MBB, MI, DL, TII->get(I4004::BBL)).addImm(0);
    MI.eraseFromParent();
    return true;

  case I4004::MOVi8: {
    // FIM Pdst, imm8.
    Register Dst = MI.getOperand(0).getReg();
    int64_t Imm = MI.getOperand(1).getImm() & 0xFF;
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(Dst).addImm(Imm);
    MI.eraseFromParent();
    return true;
  }

  case I4004::COPYpr:
    emitCopy(MBB, MI);
    MI.eraseFromParent();
    return true;

  case I4004::ADDi8:
    emitAddSub(MBB, MI, I4004::ADD);
    MI.eraseFromParent();
    return true;

  case I4004::SUBi8:
    emitAddSub(MBB, MI, I4004::SUB);
    MI.eraseFromParent();
    return true;
  }
}

bool I4004ExpandPseudos::runOnMachineFunction(MachineFunction &MF) {
  const I4004Subtarget &STI = MF.getSubtarget<I4004Subtarget>();
  TII = STI.getInstrInfo();
  TRI = STI.getRegisterInfo();

  bool Changed = false;
  for (MachineBasicBlock &MBB : MF) {
    for (auto I = MBB.begin(), E = MBB.end(); I != E;) {
      MachineInstr &MI = *I++;
      Changed |= expandMI(MBB, MI);
    }
  }
  return Changed;
}

FunctionPass *llvm::createI4004ExpandPseudosPass() {
  return new I4004ExpandPseudos();
}
