//===-- I8008HLTrack.cpp - Remove redundant H:L loads --------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// spec/codegen.md sec.5. Every memory pseudo expands to its own H:L setup:
// `lhi %hi(a); lli %lo(a)` for a link-time address, `lh<p.hi>; ll<p.lo>` for
// a pointer pair. Walking each block forward, this pass remembers what H:L
// holds and
//   * deletes a setup that loads what H:L already holds;
//   * for an address one byte above / below the current one in the same
//     page-bounded object (abi.md sec.4), keeps H and replaces `lli` with
//     `inl` / `dcl`; for any other address in that object it drops `lhi`.
// The flags are dead wherever a setup appears (expansions never put one
// between a compare and its branch), so INL/DCL may clobber them.
//
//===----------------------------------------------------------------------===//

#include "I8008.h"
#include "I8008InstrInfo.h"
#include "I8008Subtarget.h"
#include "MCTargetDesc/I8008OpTables.h"
#include "MCTargetDesc/I8008Specifiers.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/IR/GlobalVariable.h"

using namespace llvm;
using namespace llvm::I8008Op;

#define DEBUG_TYPE "i8008-hltrack"
#define PASS_NAME "I8008 H:L tracking"

namespace {

// What H:L holds.
struct HLState {
  enum { Unknown, Abs, Pair } Kind = Unknown;
  // Abs: the symbol operand (global or external symbol) and offset.
  const GlobalValue *GV = nullptr;
  StringRef ES;
  int64_t Off = 0;
  // Pair: the 16-bit register copied into H:L.
  Register P;
};

class I8008HLTrack : public MachineFunctionPass {
public:
  static char ID;
  I8008HLTrack() : MachineFunctionPass(ID) {}
  bool runOnMachineFunction(MachineFunction &MF) override;
  StringRef getPassName() const override { return PASS_NAME; }

private:
  const I8008InstrInfo *TII = nullptr;
  const TargetRegisterInfo *TRI = nullptr;
  bool runOnBlock(MachineBasicBlock &MBB);
};

char I8008HLTrack::ID = 0;

bool isMVI(const MachineInstr &MI, unsigned Field) {
  return MI.getOpcode() == MVI[Field];
}

// The symbolic operand of an `lhi %hi(..)` / `lli %lo(..)`.
bool symOf(const MachineInstr &MI, unsigned TF, const GlobalValue *&GV,
           StringRef &ES, int64_t &Off) {
  const MachineOperand &MO = MI.getOperand(0);
  if (MO.getTargetFlags() != TF)
    return false;
  if (MO.isGlobal()) {
    GV = MO.getGlobal();
    ES = StringRef();
  } else if (MO.isSymbol()) {
    GV = nullptr;
    ES = MO.getSymbolName();
  } else {
    return false;
  }
  Off = MO.getOffset();
  return true;
}

// Size of the page-bounded object a symbol names, or 0 if unknown/too big.
uint64_t objectSize(const GlobalValue *GV, StringRef ES) {
  if (!GV)
    return (ES.starts_with("__frame_") || ES.starts_with("__args_")) ? 256 : 0;
  if (GV->getName().starts_with("__args_"))
    return 256;
  const auto *V = dyn_cast<GlobalVariable>(GV);
  if (!V)
    return 0;
  uint64_t S =
      V->getDataLayout().getTypeAllocSize(V->getValueType()).getFixedValue();
  return S <= 256 ? S : 0;
}

} // namespace

INITIALIZE_PASS(I8008HLTrack, DEBUG_TYPE, PASS_NAME, false, false)

bool I8008HLTrack::runOnBlock(MachineBasicBlock &MBB) {
  bool Changed = false;
  HLState S;
  auto Lo = [&](Register R) { return TRI->getSubReg(R, I8008::sub_lo); };
  auto Hi = [&](Register R) { return TRI->getSubReg(R, I8008::sub_hi); };

  for (auto I = MBB.begin(), E = MBB.end(); I != E;) {
    MachineInstr &MI = *I;
    auto Next = std::next(I);
    if (MI.getPreInstrSymbol())
      S = HLState();

    // `lhi %hi(a); lli %lo(a)`
    if (isMVI(MI, FH) && Next != E && isMVI(*Next, FL) &&
        !Next->getPreInstrSymbol()) {
      const GlobalValue *GV, *GV2;
      StringRef ES, ES2;
      int64_t Off, Off2;
      if (symOf(MI, I8008II::MO_HI, GV, ES, Off) &&
          symOf(*Next, I8008II::MO_LO, GV2, ES2, Off2) && GV == GV2 &&
          ES == ES2 && Off == Off2) {
        uint64_t Size = objectSize(GV, ES);
        bool SameObj = S.Kind == HLState::Abs && S.GV == GV && S.ES == ES &&
                       Size && S.Off >= 0 && (uint64_t)S.Off < Size &&
                       Off >= 0 && (uint64_t)Off < Size;
        auto After = std::next(Next);
        if (S.Kind == HLState::Abs && S.GV == GV && S.ES == ES && S.Off == Off) {
          MI.eraseFromParent();
          Next->eraseFromParent();
          Changed = true;
        } else if (SameObj && (Off == S.Off + 1 || Off == S.Off - 1)) {
          BuildMI(MBB, MI, MI.getDebugLoc(),
                  TII->get(Off == S.Off + 1 ? INR[FL] : DCR[FL]));
          MI.eraseFromParent();
          Next->eraseFromParent();
          Changed = true;
        } else if (SameObj) {
          MI.eraseFromParent(); // H is unchanged within the object
          Changed = true;
        }
        S.Kind = HLState::Abs;
        S.GV = GV;
        S.ES = ES;
        S.Off = Off;
        I = After;
        continue;
      }
    }

    // `lh<p.hi>; ll<p.lo>`
    if (Next != E && !Next->getPreInstrSymbol()) {
      for (Register P : {Register(I8008::BC), Register(I8008::DE)}) {
        if (MI.getOpcode() == MOV[FH][I8008InstrInfo::regField(Hi(P))] &&
            Next->getOpcode() == MOV[FL][I8008InstrInfo::regField(Lo(P))]) {
          auto After = std::next(Next);
          if (S.Kind == HLState::Pair && S.P == P) {
            MI.eraseFromParent();
            Next->eraseFromParent();
            Changed = true;
          }
          S = HLState();
          S.Kind = HLState::Pair;
          S.P = P;
          I = After;
          goto next_instr;
        }
      }
    }

    // `inl` / `dcl` on a tracked page-bounded address.
    if ((MI.getOpcode() == INR[FL] || MI.getOpcode() == DCR[FL]) &&
        S.Kind == HLState::Abs) {
      uint64_t Size = objectSize(S.GV, S.ES);
      int64_t N = S.Off + (MI.getOpcode() == INR[FL] ? 1 : -1);
      // The next instruction may be the carry fix-up `jfz 1f; inh`, which
      // only runs when L wrapped; keep tracking only when it cannot have.
      if (Size && N >= 0 && (uint64_t)N < Size && !MI.getPostInstrSymbol()) {
        S.Off = N;
        if (Next != E && Next->getOpcode() == JCC[0][CondZ])
          S = HLState();
        I = Next;
        continue;
      }
      S = HLState();
      I = Next;
      continue;
    }

    // Anything else: a call, a write to H/L, or a write to the tracked pair
    // ends what we know.
    if (MI.isCall() || MI.getPostInstrSymbol() ||
        MI.modifiesRegister(I8008::H, TRI) ||
        MI.modifiesRegister(I8008::L, TRI) ||
        (S.Kind == HLState::Pair && MI.modifiesRegister(S.P, TRI)))
      S = HLState();
    I = Next;
  next_instr:;
  }
  return Changed;
}

bool I8008HLTrack::runOnMachineFunction(MachineFunction &MF) {
  const I8008Subtarget &STI = MF.getSubtarget<I8008Subtarget>();
  TII = STI.getInstrInfo();
  TRI = STI.getRegisterInfo();
  bool Changed = false;
  for (MachineBasicBlock &MBB : MF)
    Changed |= runOnBlock(MBB);
  return Changed;
}

FunctionPass *llvm::createI8008HLTrackPass() { return new I8008HLTrack(); }
