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

      // DCL changes the CM-RAM (DCL) bank. The SRC address latch is NOT a
      // single CPU register: each 4002 chip holds its OWN {reg,char} latch, and
      // which chip answers an RDM/WRM/RDn/WRn is gated by the CM-RAM bank line
      // the last DCL drove (datasheet; ai_hw_engineer intel_4002.sv guards both
      // the SRC load and the response with cm_ram_i). So a `SRC pN` issued
      // under DCL=k latches the address ONLY into bank k's chip; after a later
      // `DCL j` (j != k) the bank-j chip that now answers may hold a STALE/old
      // address, and a bank-k access after returning to k relies on k's chip
      // still holding what the elided SRC would have re-set. Tracking "SRC
      // points at pair pN" as if it were bank-independent is therefore wrong
      // across a bank change: a `SRC pN` after a DCL must NOT be elided even if
      // pN is unchanged, because the newly-selected chip never received it.
      //
      // This is the prime suspect for the deep banked-frame corruption
      // (failure_notes/2026-10-19-chips-4002-src-latch-ignores-dcl.md): the
      // DCL frame brackets (I4004ExpandPseudos emits `LD bank; DCL; SRC; I/O;
      // LDM 0; DCL`) run before this post-expansion pass, and a bank-0 SRC
      // after a banked frame's `DCL 0` reset was being dropped because SrcPair
      // still named the pair from before the bracket. Invalidate the SRC latch
      // on every DCL. Register CONTENTS survive a DCL, so PairImm is untouched;
      // only the hardware address-latch tracking (SrcPair) is cleared.
      if (Op == I4004::DCL) {
        SrcPair = Register();
        continue;
      }

      // Control transfer that leaves straight-line flow mid-block:
      //   * JMS -- a call to a leaf helper (push/pop, HAL, regops) that does its
      //     own SRC/DCL and returns with the latch in an unknown state;
      //   * JUN / JIN -- soft-call dispatch and the `jin p0` return stubs, which
      //     transfer into code (__soft_ret, __push_*, restore trampolines, far
      //     thunks) that re-SRCs and may change the DCL bank;
      //   * BBL / RETpseudo -- return out of this function.
      // In every case the SRC latch and any known pair immediates cannot be
      // assumed across the transfer. This mirrors the existing isCall() clear
      // below, extended to the i4004's non-isCall control transfers (the
      // soft-call ABI uses JUN, not the hardware-stack JMS, for real calls --
      // abi.md 4.4.1 -- so isCall() alone does not cover them). INLINEASM is
      // opaque: it may issue arbitrary SRC/DCL, so treat it the same.
      if (Op == I4004::JMS || Op == I4004::JUN || Op == I4004::JIN ||
          Op == I4004::BBL || Op == I4004::RETpseudo ||
          MI.isInlineAsm()) {
        PairImm.clear();
        SrcPair = Register();
        // JMS/JUN with an argument pair still "uses" registers but defines
        // nothing we track; fall through is unnecessary -- nothing below
        // applies to these opcodes.
        continue;
      }

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
        // Unknown form (e.g. FIM pN, <symbol> for a per-function frame base,
        // abi.md 4.4): the pair now holds a relocated address we cannot track,
        // so it is NO LONGER a known immediate AND SRC's latch for that pair is
        // stale. FIM models its pair as an INPUT operand ((outs) is empty), so
        // handleDefs -- which keys on isDef() -- would NOT invalidate it; do it
        // explicitly here, else a following `SRC pN` is wrongly elided (it was,
        // dropping the leading SRC of a symbol-addressed spill and corrupting
        // every per-function-frame store/reload).
        if (PR)
          invalidatePair(PR);
        else
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
