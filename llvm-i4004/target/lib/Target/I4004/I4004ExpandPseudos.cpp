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
#include "I4004ISelLowering.h"
#include "I4004Subtarget.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include <array>

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
  void emitAddSub16(MachineBasicBlock &MBB, MachineInstr &MI, unsigned AluOpc);
  void emitCopy(MachineBasicBlock &MBB, MachineInstr &MI);
  void emitBrCC(MachineBasicBlock &MBB, MachineInstr &MI, bool Wide);
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

// 16-bit add/sub over a DR16: four nibbles low->high, carry chained. SUB flips
// the carry polarity with CMC between every nibble (same reason as emitAddSub).
void I4004ExpandPseudos::emitAddSub16(MachineBasicBlock &MBB, MachineInstr &MI,
                                      unsigned AluOpc) {
  const DebugLoc &DL = MI.getDebugLoc();
  Register Dst = MI.getOperand(0).getReg();
  Register Lhs = MI.getOperand(1).getReg();
  Register Rhs = MI.getOperand(2).getReg();
  const bool IsSub = (AluOpc == I4004::SUB);

  auto nibsOf = [&](Register D) -> std::array<Register, 4> {
    Register LoP = TRI->getSubReg(D, I4004::sub_pair_lo);
    Register HiP = TRI->getSubReg(D, I4004::sub_pair_hi);
    return {TRI->getSubReg(LoP, I4004::sub_lo),
            TRI->getSubReg(LoP, I4004::sub_hi),
            TRI->getSubReg(HiP, I4004::sub_lo),
            TRI->getSubReg(HiP, I4004::sub_hi)};
  };
  auto DN = nibsOf(Dst), LN = nibsOf(Lhs), RN = nibsOf(Rhs);

  BuildMI(MBB, MI, DL, TII->get(I4004::CLC));
  for (int i = 0; i < 4; ++i) {
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(LN[i]);
    BuildMI(MBB, MI, DL, TII->get(AluOpc)).addReg(RN[i]);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DN[i], RegState::Define);
    if (IsSub && i != 3)
      BuildMI(MBB, MI, DL, TII->get(I4004::CMC));
  }
}

// Compare-and-branch. Operands: lhs, rhs, cc(imm), dest(MBB). EQ/NE compare
// each nibble independently (CLC;LD;SUB;JCN Z/NZ); the relational codes do a
// full SUB chain (CMC between nibbles) and test the final borrow in CY.
// JCN condition nibble (isa.md sec.4): C1=invert, C2=Z(ACC==0), C3=CY. So
// Z=0x4, NZ=0xC, CY-set=0x2, CY-clear=0xA.
void I4004ExpandPseudos::emitBrCC(MachineBasicBlock &MBB, MachineInstr &MI,
                                  bool Wide) {
  const DebugLoc &DL = MI.getDebugLoc();
  Register Lhs = MI.getOperand(0).getReg();
  Register Rhs = MI.getOperand(1).getReg();
  int CC = MI.getOperand(2).getImm();
  MachineBasicBlock *Dest = MI.getOperand(3).getMBB();

  auto nibsI8 = [&](Register P) -> SmallVector<Register, 4> {
    return {TRI->getSubReg(P, I4004::sub_lo), TRI->getSubReg(P, I4004::sub_hi)};
  };
  auto nibsI16 = [&](Register D) -> SmallVector<Register, 4> {
    Register LoP = TRI->getSubReg(D, I4004::sub_pair_lo);
    Register HiP = TRI->getSubReg(D, I4004::sub_pair_hi);
    return {TRI->getSubReg(LoP, I4004::sub_lo),
            TRI->getSubReg(LoP, I4004::sub_hi),
            TRI->getSubReg(HiP, I4004::sub_lo),
            TRI->getSubReg(HiP, I4004::sub_hi)};
  };
  SmallVector<Register, 4> LN = Wide ? nibsI16(Lhs) : nibsI8(Lhs);
  SmallVector<Register, 4> RN = Wide ? nibsI16(Rhs) : nibsI8(Rhs);
  const unsigned N = LN.size();

  auto jcn = [&](int cond, MachineBasicBlock *T) {
    BuildMI(MBB, MI, DL, TII->get(I4004::JCN)).addImm(cond).addMBB(T);
  };

  if (CC == I4004CC::EQ || CC == I4004CC::NE) {
    // Per-nibble independent equality. NE: branch if ANY nibble differs.
    // EQ: for all but the last nibble, a nonzero nibble means "not equal" so
    // jump to the fall-through successor; the last uses JCN Z to dest.
    MachineBasicBlock *FallThru = nullptr;
    for (MachineBasicBlock *S : MBB.successors())
      if (S != Dest)
        FallThru = S;
    // These MBBs are reached by our late-inserted JCNs; force their labels so
    // the AsmPrinter emits them even when one is the layout fall-through.
    Dest->setLabelMustBeEmitted();
    if (FallThru)
      FallThru->setLabelMustBeEmitted();
    for (unsigned i = 0; i < N; ++i) {
      BuildMI(MBB, MI, DL, TII->get(I4004::CLC));
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(LN[i]);
      BuildMI(MBB, MI, DL, TII->get(I4004::SUB)).addReg(RN[i]);
      if (CC == I4004CC::NE) {
        jcn(0xC, Dest); // NZ -> this nibble differs -> a != b
      } else { // EQ
        if (i + 1 < N) {
          if (FallThru)
            jcn(0xC, FallThru); // nonzero here -> not equal -> fall through
        } else {
          jcn(0x4, Dest); // last nibble zero and all prior zero -> equal
        }
      }
    }
    return;
  }

  // Relational: full SUB chain, final CY = unsigned (lhs >= rhs). CMC between
  // nibbles (same borrow-polarity fix as emitAddSub).
  BuildMI(MBB, MI, DL, TII->get(I4004::CLC));
  for (unsigned i = 0; i < N; ++i) {
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(LN[i]);
    BuildMI(MBB, MI, DL, TII->get(I4004::SUB)).addReg(RN[i]);
    if (i + 1 < N)
      BuildMI(MBB, MI, DL, TII->get(I4004::CMC));
  }
  // After the chain CY=1 means no borrow => lhs >= rhs (unsigned). UGT/ULE were
  // canonicalized to ULT/UGE by operand swap in LowerBR_CC.
  switch (CC) {
  case I4004CC::UGE: jcn(0x2, Dest); break; // CY=1
  case I4004CC::ULT: jcn(0xA, Dest); break; // CY=0 (invert|CY)
  default:
    report_fatal_error("I4004: unexpected BRCC relational code after "
                       "canonicalization");
  }
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

  case I4004::CALL:
    // Call to a known address: JMS <target> (hardware stack).
    BuildMI(MBB, MI, DL, TII->get(I4004::JMS)).add(MI.getOperand(0));
    MI.eraseFromParent();
    return true;

  case I4004::ZEXT8TO16:
  case I4004::ANYEXT8TO16: {
    // dst(i16) low sub-pair = x (i8); high sub-pair = 0.
    Register Dst = MI.getOperand(0).getReg();
    Register X = MI.getOperand(1).getReg();
    Register DLoP = TRI->getSubReg(Dst, I4004::sub_pair_lo);
    Register DHiP = TRI->getSubReg(Dst, I4004::sub_pair_hi);
    // low pair = x, via real nibble moves (LD/XCH).
    Register DHi = TRI->getSubReg(DLoP, I4004::sub_hi);
    Register DLo = TRI->getSubReg(DLoP, I4004::sub_lo);
    Register XHi = TRI->getSubReg(X, I4004::sub_hi);
    Register XLo = TRI->getSubReg(X, I4004::sub_lo);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(XHi);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DHi, RegState::Define);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(XLo);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DLo, RegState::Define);
    // high pair = 0.
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(DHiP).addImm(0);
    MI.eraseFromParent();
    return true;
  }

  case I4004::MOVi8: {
    // FIM Pdst, imm8.
    Register Dst = MI.getOperand(0).getReg();
    int64_t Imm = MI.getOperand(1).getImm() & 0xFF;
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(Dst).addImm(Imm);
    MI.eraseFromParent();
    return true;
  }

  case I4004::MOVi16: {
    // FIM into both sub-pairs: low pair = imm[7:0], high pair = imm[15:8].
    Register Dst = MI.getOperand(0).getReg();
    int64_t Imm = MI.getOperand(1).getImm() & 0xFFFF;
    Register LoP = TRI->getSubReg(Dst, I4004::sub_pair_lo);
    Register HiP = TRI->getSubReg(Dst, I4004::sub_pair_hi);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(LoP).addImm(Imm & 0xFF);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(HiP).addImm((Imm >> 8) & 0xFF);
    MI.eraseFromParent();
    return true;
  }

  case I4004::LEA_FI: {
    // dst(DR16) = RAM address of the slot (operand1 = resolved char addr, low
    // byte); high byte 0.
    Register Dst = MI.getOperand(0).getReg();
    int64_t Addr = MI.getOperand(1).getImm() & 0xFF;
    Register LoP = TRI->getSubReg(Dst, I4004::sub_pair_lo);
    Register HiP = TRI->getSubReg(Dst, I4004::sub_pair_hi);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(LoP).addImm(Addr);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(HiP).addImm(0);
    MI.eraseFromParent();
    return true;
  }

  case I4004::COPYpr:
    emitCopy(MBB, MI);
    MI.eraseFromParent();
    return true;

  case I4004::BR: {
    // Unconditional branch -> JUN target (reaches the whole 12-bit space;
    // page-range checking is a linker concern -- abi.md sec.5).
    BuildMI(MBB, MI, DL, TII->get(I4004::JUN)).add(MI.getOperand(0));
    MI.eraseFromParent();
    return true;
  }

  case I4004::BRZ:
  case I4004::BRNZ: {
    // JCN condition nibble bits (isa.md sec.4): C1=invert, C2=ACC==0(Z). So
    // Z -> 0x4 (branch if ACC==0), NZ -> 0xC (invert|Z).
    const bool BrNZ = (MI.getOpcode() == I4004::BRNZ);
    Register Val = MI.getOperand(0).getReg();
    MachineOperand Dst = MI.getOperand(1);
    auto [Hi, Lo] = nibbles(Val);
    if (BrNZ) {
      // Taken if any nibble nonzero: JCN NZ on lo, then JCN NZ on hi.
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Lo);
      BuildMI(MBB, MI, DL, TII->get(I4004::JCN)).addImm(0xC).add(Dst);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Hi);
      BuildMI(MBB, MI, DL, TII->get(I4004::JCN)).addImm(0xC).add(Dst);
    } else {
      // Backbone BRZ assumes a boolean i8 (0/1): the low nibble suffices.
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Lo);
      BuildMI(MBB, MI, DL, TII->get(I4004::JCN)).addImm(0x4).add(Dst);
    }
    MI.eraseFromParent();
    return true;
  }

  case I4004::ADDi8:
    emitAddSub(MBB, MI, I4004::ADD);
    MI.eraseFromParent();
    return true;

  case I4004::SUBi8:
    emitAddSub(MBB, MI, I4004::SUB);
    MI.eraseFromParent();
    return true;

  case I4004::ADDi16:
    emitAddSub16(MBB, MI, I4004::ADD);
    MI.eraseFromParent();
    return true;

  case I4004::SUBi16:
    emitAddSub16(MBB, MI, I4004::SUB);
    MI.eraseFromParent();
    return true;

  case I4004::LDRAMi8: {
    // dst(i8) = RAM[ptr]. ptr is a DR16; its low sub-pair (sub_pair_lo) is the
    // SRC address byte, whose low nibble (sub_lo) is the char index.
    Register Dst = MI.getOperand(0).getReg();
    Register Ptr = MI.getOperand(1).getReg();
    Register LoPair = TRI->getSubReg(Ptr, I4004::sub_pair_lo);
    Register CharReg = TRI->getSubReg(LoPair, I4004::sub_lo);
    auto [DHi, DLo] = nibbles(Dst);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(LoPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DLo, RegState::Define);
    BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(CharReg);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(LoPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DHi, RegState::Define);
    // Restore the char index (DAC = ACC-1) so the pointer is unchanged.
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(CharReg);
    BuildMI(MBB, MI, DL, TII->get(I4004::DAC));
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(CharReg, RegState::Define);
    MI.eraseFromParent();
    return true;
  }

  case I4004::STRAMi8: {
    // RAM[ptr] = val(i8). Same addressing as LDRAMi8.
    Register Val = MI.getOperand(0).getReg();
    Register Ptr = MI.getOperand(1).getReg();
    Register LoPair = TRI->getSubReg(Ptr, I4004::sub_pair_lo);
    Register CharReg = TRI->getSubReg(LoPair, I4004::sub_lo);
    auto [VHi, VLo] = nibbles(Val);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(LoPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(VLo);
    BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
    BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(CharReg);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(LoPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(VHi);
    BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(CharReg);
    BuildMI(MBB, MI, DL, TII->get(I4004::DAC));
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(CharReg, RegState::Define);
    MI.eraseFromParent();
    return true;
  }

  case I4004::LDRAMi16:
  case I4004::STRAMi16: {
    // i16 RAM access: four consecutive chars, low byte first. Copy the pointer
    // byte into P0 so we can INC it 0..3 without disturbing the pointer, then
    // SRC P0 / RDM|WRM each char. Value halves: low pair = chars 0,1; high pair
    // = chars 2,3 (nibble order low,hi within each byte).
    const bool IsLoad = (MI.getOpcode() == I4004::LDRAMi16);
    Register ValDR = MI.getOperand(0).getReg();
    Register Ptr = MI.getOperand(1).getReg();
    Register PtrLoPair = TRI->getSubReg(Ptr, I4004::sub_pair_lo);
    // P0 = copy of the pointer's low byte (address), via LD/XCH nibble moves.
    Register P0Hi = TRI->getSubReg(I4004::P0, I4004::sub_hi);
    Register P0Lo = TRI->getSubReg(I4004::P0, I4004::sub_lo);
    Register PHi = TRI->getSubReg(PtrLoPair, I4004::sub_hi);
    Register PLo = TRI->getSubReg(PtrLoPair, I4004::sub_lo);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PHi);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(P0Hi, RegState::Define);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PLo);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(P0Lo, RegState::Define);
    Register ValLoPair = TRI->getSubReg(ValDR, I4004::sub_pair_lo);
    Register ValHiPair = TRI->getSubReg(ValDR, I4004::sub_pair_hi);
    Register Nib[4] = {TRI->getSubReg(ValLoPair, I4004::sub_lo),
                       TRI->getSubReg(ValLoPair, I4004::sub_hi),
                       TRI->getSubReg(ValHiPair, I4004::sub_lo),
                       TRI->getSubReg(ValHiPair, I4004::sub_hi)};
    for (int i = 0; i < 4; ++i) {
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      if (IsLoad) {
        BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
        BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(Nib[i], RegState::Define);
      } else {
        BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Nib[i]);
        BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
      }
      if (i != 3)
        BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(P0Lo);
    }
    MI.eraseFromParent();
    return true;
  }

  case I4004::BRCCi8:
    emitBrCC(MBB, MI, /*Wide=*/false);
    MI.eraseFromParent();
    return true;

  case I4004::BRCCi16:
    emitBrCC(MBB, MI, /*Wide=*/true);
    MI.eraseFromParent();
    return true;

  case I4004::SPILL8:
  case I4004::SPILL16: {
    // RAM[spillslot] = src. Operand1 is the resolved char-base immediate.
    Register Src = MI.getOperand(0).getReg();
    int64_t Base = MI.getOperand(1).getImm() & 0xFF;
    const bool Wide = (MI.getOpcode() == I4004::SPILL16);
    // Collect the byte-pairs to store (1 for i8, 2 for i16), each as {hi,lo}.
    SmallVector<Register, 2> Pairs;
    if (Wide) {
      Pairs.push_back(TRI->getSubReg(Src, I4004::sub_pair_lo));
      Pairs.push_back(TRI->getSubReg(Src, I4004::sub_pair_hi));
    } else {
      Pairs.push_back(Src);
    }
    int64_t Char = Base;
    for (Register P : Pairs) {
      Register PHi = TRI->getSubReg(P, I4004::sub_hi);
      Register PLo = TRI->getSubReg(P, I4004::sub_lo);
      // low nibble -> Char, high nibble -> Char+1.
      BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(I4004::P0).addImm(Char & 0xFF);
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PLo);
      BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
      BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(I4004::P0).addImm((Char + 1) & 0xFF);
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PHi);
      BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
      Char += 2;
    }
    MI.eraseFromParent();
    return true;
  }

  case I4004::RELOAD8:
  case I4004::RELOAD16: {
    Register Dst = MI.getOperand(0).getReg();
    int64_t Base = MI.getOperand(1).getImm() & 0xFF;
    const bool Wide = (MI.getOpcode() == I4004::RELOAD16);
    SmallVector<Register, 2> Pairs;
    if (Wide) {
      Pairs.push_back(TRI->getSubReg(Dst, I4004::sub_pair_lo));
      Pairs.push_back(TRI->getSubReg(Dst, I4004::sub_pair_hi));
    } else {
      Pairs.push_back(Dst);
    }
    int64_t Char = Base;
    for (Register P : Pairs) {
      Register PHi = TRI->getSubReg(P, I4004::sub_hi);
      Register PLo = TRI->getSubReg(P, I4004::sub_lo);
      BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(I4004::P0).addImm(Char & 0xFF);
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(PLo, RegState::Define);
      BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(I4004::P0).addImm((Char + 1) & 0xFF);
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(PHi, RegState::Define);
      Char += 2;
    }
    MI.eraseFromParent();
    return true;
  }
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
