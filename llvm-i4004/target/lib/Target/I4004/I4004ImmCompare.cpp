//===-- I4004ImmCompare.cpp - compare-with-constant branches -------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Pre-RA (SSA) pass (issue #18, A3). A BRCCi8/BRCCi16 whose operand is a
// MOVi8/MOVi16 constant -- from BR_CC or a SELECT_CC diamond -- becomes
// BRCCi8ri/BRCCi16ri, which reads the constant through LDM (`LDM c; SUB x`
// computes c - x). The constant then occupies no register pair, which on a
// machine with six allocatable pairs is what decides whether the function
// spills. The relational codes are canonicalised for the c - x direction:
//
//   x == c, x != c            EQ / NE
//   x <= c (u), x > c (u)     ULE / UGT   (CY after c - x)
//   x <  c (u), c > 0         ULE c-1        x >= c (u), c > 0   UGT c-1
//   x <  0 (s), x <= -1 (s)   NEG            x >= 0 (s), x > -1  NONNEG
//
// Other signed relations against a constant keep the register form. A MOVi
// left without users is deleted.
//
//===----------------------------------------------------------------------===//

#include "I4004.h"
#include "I4004ISelLowering.h"
#include "I4004InstrInfo.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-imm-compare"

namespace {
class I4004ImmCompare : public MachineFunctionPass {
public:
  static char ID;
  I4004ImmCompare() : MachineFunctionPass(ID) {}
  bool runOnMachineFunction(MachineFunction &MF) override;
  StringRef getPassName() const override {
    return "I4004 compare-with-constant branches";
  }
};
} // namespace

char I4004ImmCompare::ID = 0;

INITIALIZE_PASS(I4004ImmCompare, DEBUG_TYPE,
                "I4004 compare-with-constant branches", false, false)

// The constant a virtual register holds, if it is defined by MOVi8/MOVi16.
static std::optional<int64_t> constOf(const MachineRegisterInfo &MRI,
                                      Register R) {
  if (!R.isVirtual())
    return std::nullopt;
  const MachineInstr *Def = MRI.getVRegDef(R);
  if (!Def || (Def->getOpcode() != I4004::MOVi8 &&
               Def->getOpcode() != I4004::MOVi16))
    return std::nullopt;
  return Def->getOperand(1).getImm();
}

// Signed relations that only arise from swapping a constant lhs.
enum : int { SGT_ = 100, SLE_ = 101 };

// The relation with operands swapped: (c op x) == (x swap(op) c).
static int swapCC(int CC) {
  switch (CC) {
  case I4004CC::ULT: return I4004CC::UGT;
  case I4004CC::UGE: return I4004CC::ULE;
  case I4004CC::UGT: return I4004CC::ULT;
  case I4004CC::ULE: return I4004CC::UGE;
  case I4004CC::SLT: return SGT_;
  case I4004CC::SGE: return SLE_;
  default:           return CC; // EQ, NE
  }
}

bool I4004ImmCompare::runOnMachineFunction(MachineFunction &MF) {
  MachineRegisterInfo &MRI = MF.getRegInfo();
  const TargetInstrInfo &TII = *MF.getSubtarget().getInstrInfo();
  bool Changed = false;
  SmallVector<Register, 8> MaybeDead;

  for (MachineBasicBlock &MBB : MF) {
    for (MachineInstr &MI : llvm::make_early_inc_range(MBB)) {
      unsigned Opc = MI.getOpcode();
      if (Opc != I4004::BRCCi8 && Opc != I4004::BRCCi16)
        continue;
      const bool Wide = Opc == I4004::BRCCi16;
      const int64_t Mask = Wide ? 0xFFFF : 0xFF;

      Register L = MI.getOperand(0).getReg();
      Register R = MI.getOperand(1).getReg();
      const int Used = MI.getOperand(2).getImm() & 0xF0;
      int CC = I4004CC::code(MI.getOperand(2).getImm());
      Register X;
      int64_t C;
      if (auto RC = constOf(MRI, R)) {
        X = L;
        C = *RC & Mask;
      } else if (auto LC = constOf(MRI, L)) {
        X = R;
        C = *LC & Mask;
        CC = swapCC(CC);
      } else {
        continue;
      }
      int NewCC;
      int64_t NewC = C;
      switch (CC) {
      case I4004CC::EQ:
      case I4004CC::NE:
      case I4004CC::ULE:
      case I4004CC::UGT:
        NewCC = CC;
        break;
      case I4004CC::ULT: // x < c  ==  x <= c-1
        if (C == 0)
          continue;
        NewCC = I4004CC::ULE;
        NewC = C - 1;
        break;
      case I4004CC::UGE: // x >= c  ==  x > c-1
        if (C == 0)
          continue;
        NewCC = I4004CC::UGT;
        NewC = C - 1;
        break;
      case I4004CC::SLT: // x < 0
        if (C != 0)
          continue;
        NewCC = I4004CC::NEG;
        break;
      case I4004CC::SGE: // x >= 0
        if (C != 0)
          continue;
        NewCC = I4004CC::NONNEG;
        break;
      case SGT_: // x > -1  ==  x >= 0
        if (C != Mask)
          continue;
        NewCC = I4004CC::NONNEG;
        break;
      case SLE_: // x <= -1  ==  x < 0
        if (C != Mask)
          continue;
        NewCC = I4004CC::NEG;
        break;
      default:
        continue;
      }

      // A 4002 character tested for (in)equality right after it is read:
      // test ACC instead of moving the nibble into a pair (BRRAM).
      if (!Wide && (NewCC == I4004CC::EQ || NewCC == I4004CC::NE) &&
          NewC < 16 && X.isVirtual() && MRI.hasOneNonDBGUse(X)) {
        MachineInstr *Rd = MRI.getVRegDef(X);
        bool Fuse = Rd && Rd->getParent() == &MBB &&
                    (Rd->getOpcode() == I4004::RAMRS ||
                     Rd->getOpcode() == I4004::RAMRD);
        if (Fuse) {
          for (auto I = std::next(Rd->getIterator()); &*I != &MI; ++I)
            if (I->mayStore() || I->isCall() || I->hasUnmodeledSideEffects()) {
              Fuse = false;
              break;
            }
        }
        if (Fuse) {
          int64_t K = Rd->getOpcode() == I4004::RAMRS ? Rd->getOperand(2).getImm()
                                                      : 4;
          BuildMI(MBB, MI, MI.getDebugLoc(), TII.get(I4004::BRRAM))
              .add(Rd->getOperand(1))
              .addImm(K | (NewC << 4))
              .addImm(NewCC)
              .add(MI.getOperand(3));
          MaybeDead.push_back(X == L ? R : L);
          MI.eraseFromParent();
          Rd->eraseFromParent();
          Changed = true;
          continue;
        }
      }
      BuildMI(MBB, MI, MI.getDebugLoc(),
              TII.get(Wide ? I4004::BRCCi16ri : I4004::BRCCi8ri))
          .addReg(X)
          .addImm(NewC & Mask)
          .addImm(NewCC | (NewCC == I4004CC::NEG || NewCC == I4004CC::NONNEG
                               ? 0
                               : Used))
          .add(MI.getOperand(3));
      MaybeDead.push_back(X == L ? R : L);
      MI.eraseFromParent();
      Changed = true;
    }
  }
  for (Register R : MaybeDead) {
    if (!MRI.use_nodbg_empty(R))
      continue;
    if (MachineInstr *Def = MRI.getVRegDef(R))
      Def->eraseFromParent();
  }
  return Changed;
}

FunctionPass *llvm::createI4004ImmComparePass() {
  return new I4004ImmCompare();
}
