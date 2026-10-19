//===-- I4004Peephole.cpp - redundant FIM/SRC elimination ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// A post-expansion peephole that removes the single largest source of code
// bloat in the i4004 backend: redundant pointer-address setup. Profiling the
// whole Busicom firmware showed `FIM pN,<const>` is ~33% of the image and
// there are ~5,400 `FIM; SRC` pointer setups, many of them re-materializing an
// address a register already holds and re-SRCing a pair SRC already points at.
//
// Within a single basic block this pass tracks, with a simple forward scan:
//   * for each register PAIR, the immediate it was last loaded with by a
//     `FIM pN, imm` (invalidated when any sub-register of the pair, or the pair
//     itself, is redefined by anything else);
//   * which pair the hardware SRC address latch currently holds (set by `SRC
//     pN`, invalidated when that pair's value changes).
// It then deletes:
//   * a `FIM pN, imm` whose pair already provably holds `imm`;
//   * a `SRC pN` when SRC already points at pN and pN is unchanged since.
//
// Conservative: any instruction that defines a tracked pair's sub-registers
// clears that pair's known-immediate (and SRC, if it pointed there); calls and
// anything that could change SRC clear the SRC state. Block boundaries reset
// everything (no cross-block assumptions). This never changes behaviour — it
// only elides provably-redundant address loads.
//
//===----------------------------------------------------------------------===//

#include "I4004.h"
#include "I4004InstrInfo.h"
#include "I4004RegisterInfo.h"
#include "I4004Subtarget.h"
#include "llvm/ADT/DenseMap.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include "llvm/CodeGen/TargetRegisterInfo.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-peephole"

namespace {
class I4004Peephole : public MachineFunctionPass {
public:
  static char ID;
  I4004Peephole() : MachineFunctionPass(ID) {}

  bool runOnMachineFunction(MachineFunction &MF) override;

  StringRef getPassName() const override {
    return "I4004 redundant FIM/SRC peephole";
  }

private:
  const TargetRegisterInfo *TRI = nullptr;
};
} // end anonymous namespace

char I4004Peephole::ID = 0;

// Return the pair register (P0..P7) that register R belongs to, or 0.
static Register pairOf(Register R, const TargetRegisterInfo *TRI) {
  for (MCPhysReg P : {I4004::P0, I4004::P1, I4004::P2, I4004::P3, I4004::P4,
                      I4004::P5, I4004::P6, I4004::P7}) {
    if (R == P)
      return P;
    if (TRI->isSubRegister(P, R))
      return P;
  }
  return Register();
}

bool I4004Peephole::runOnMachineFunction(MachineFunction &MF) {
  TRI = MF.getSubtarget().getRegisterInfo();
  bool Changed = false;

  for (MachineBasicBlock &MBB : MF) {
    // Known immediate currently held by each pair (by physreg), and the pair
    // SRC currently points at (0 = unknown).
    DenseMap<Register, int64_t> PairImm;
    Register SrcPair;

    auto invalidatePair = [&](Register Pair) {
      PairImm.erase(Pair);
      if (SrcPair == Pair)
        SrcPair = Register();
    };

    // On any def of a physreg, invalidate the pair it belongs to.
    auto handleDefs = [&](MachineInstr &MI, MachineInstr *Skip) {
      if (&MI == Skip)
        return;
      for (const MachineOperand &MO : MI.operands()) {
        if (!MO.isReg() || !MO.isDef() || !MO.getReg().isPhysical())
          continue;
        Register Pair = pairOf(MO.getReg(), TRI);
        if (Pair)
          invalidatePair(Pair);
      }
      // A call may clobber everything and definitely changes SRC state.
      if (MI.isCall()) {
        PairImm.clear();
        SrcPair = Register();
      }
    };

    for (auto It = MBB.begin(), E = MBB.end(); It != E;) {
      MachineInstr &MI = *It++;
      unsigned Op = MI.getOpcode();

      if (Op == I4004::FIM) {
        // FIM pN, imm8  (operands: def pair, imm).
        Register Pair = MI.getOperand(0).getReg();
        Register PR = pairOf(Pair, TRI);
        int64_t Imm = MI.getOperand(1).isImm() ? MI.getOperand(1).getImm() : -1;
        if (PR && Imm >= 0) {
          auto F = PairImm.find(PR);
          if (F != PairImm.end() && F->second == Imm) {
            // Pair already holds this immediate: delete the redundant FIM.
            MI.eraseFromParent();
            Changed = true;
            continue;
          }
          // Record the new known value (this def does not invalidate itself).
          PairImm[PR] = Imm;
          // SRC latch is unaffected by FIM to a DIFFERENT pair; if FIM targets
          // the pair SRC points at, SRC's address is now stale.
          if (SrcPair == PR)
            SrcPair = Register();
          continue;
        }
        // Unknown form: fall through to generic def handling.
        handleDefs(MI, nullptr);
        continue;
      }

      if (Op == I4004::SRC) {
        Register Pair = MI.getOperand(0).getReg();
        Register PR = pairOf(Pair, TRI);
        if (PR && SrcPair == PR) {
          // SRC already points at this pair and the pair is unchanged.
          MI.eraseFromParent();
          Changed = true;
          continue;
        }
        SrcPair = PR;
        continue;
      }

      // Generic instruction: invalidate pairs it defines, handle calls.
      handleDefs(MI, nullptr);

      // INC/ISZ increment their index-register operand in place, but the .td
      // models that register as a USE (no def operand), so handleDefs misses
      // it. Treat it as redefining the pair the register belongs to, else a
      // later SRC of that pair would be wrongly elided after the address moved.
      if (Op == I4004::INC || Op == I4004::ISZ) {
        for (const MachineOperand &MO : MI.operands()) {
          if (MO.isReg() && MO.getReg().isPhysical()) {
            if (Register Pair = pairOf(MO.getReg(), TRI))
              invalidatePair(Pair);
          }
        }
      }
    }
  }
  return Changed;
}

INITIALIZE_PASS(I4004Peephole, DEBUG_TYPE, "I4004 redundant FIM/SRC peephole",
                false, false)

namespace llvm {
FunctionPass *createI4004PeepholePass() { return new I4004Peephole(); }
void initializeI4004PeepholePass(PassRegistry &);
} // namespace llvm
