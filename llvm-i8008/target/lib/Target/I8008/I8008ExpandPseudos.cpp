//===-- I8008ExpandPseudos.cpp - Expand I8008 pseudo-instructions --------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Post-RA expansion of the CodeGen pseudos (spec/codegen.md sec.3). The 8008
// computes only in A and reaches memory only through H:L; A, H, L and the
// flags are reserved and dead between pseudos, so every expansion may use them
// as scratch. Each case notes its byte cost.
//
//===----------------------------------------------------------------------===//

#include "I8008.h"
#include "I8008ISelLowering.h"
#include "I8008InstrInfo.h"
#include "I8008Subtarget.h"
#include "MCTargetDesc/I8008OpTables.h"
#include "MCTargetDesc/I8008Specifiers.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/IR/GlobalVariable.h"
#include "llvm/MC/MCContext.h"

using namespace llvm;
using namespace llvm::I8008Op;

#define DEBUG_TYPE "i8008-expand-pseudos"
#define PASS_NAME "I8008 pseudo-instruction expansion"

namespace {

class I8008ExpandPseudos : public MachineFunctionPass {
public:
  static char ID;
  I8008ExpandPseudos() : MachineFunctionPass(ID) {}
  bool runOnMachineFunction(MachineFunction &MF) override;
  StringRef getPassName() const override { return PASS_NAME; }

private:
  const I8008InstrInfo *TII = nullptr;
  const TargetRegisterInfo *TRI = nullptr;
  MachineFunction *MF = nullptr;
  MachineBasicBlock *MBB = nullptr;
  MachineBasicBlock::iterator Pos;
  DebugLoc DL;

  MachineInstrBuilder emit(unsigned Opc) {
    return BuildMI(*MBB, Pos, DL, TII->get(Opc));
  }
  static unsigned f(Register R) { return I8008InstrInfo::regField(R); }
  Register lo(Register R) const { return TRI->getSubReg(R, I8008::sub_lo); }
  Register hi(Register R) const { return TRI->getSubReg(R, I8008::sub_hi); }

  void mov(unsigned D, unsigned S) {
    if (D != S)
      emit(MOV[D][S]);
  }
  void movR(Register D, Register S) { mov(f(D), f(S)); }
  void mviR(Register D, int64_t K) { emit(MVI[f(D)]).addImm(K & 0xff); }
  void alu(unsigned Op, Register S) { emit(ALU[Op][f(S)]); }
  void aluI(unsigned Op, int64_t K) { emit(ALUI[Op]).addImm(K & 0xff); }
  void jcc(bool True, unsigned Cond, const MachineOperand &Dst) {
    emit(JCC[True][Cond]).add(Dst);
  }
  void jccSym(bool True, unsigned Cond, MCSymbol *S) {
    emit(JCC[True][Cond]).addSym(S);
  }
  MCSymbol *label() { return MF->getContext().createTempSymbol(); }
  // Put Sym right after the last instruction emitted.
  void bindAfterLast(MCSymbol *Sym) {
    std::prev(Pos)->setPostInstrSymbol(*MF, Sym);
  }

  // A symbol operand (global / external symbol / block address) + Off,
  // lowered as %lo / %hi (TF).
  static MachineOperand symOp(const MachineOperand &Base, int64_t Off,
                              unsigned TF) {
    MachineOperand MO = Base;
    MO.setTargetFlags(TF);
    MO.setOffset(Base.getOffset() + Off);
    return MO;
  }
  // H:L <- a link-time address (memabs operands Base, Off).
  void setHLAbs(const MachineOperand &Base, int64_t Off) {
    emit(MVI[FH]).add(symOp(Base, Off, I8008II::MO_HI));
    emit(MVI[FL]).add(symOp(Base, Off, I8008II::MO_LO));
  }
  // H:L <- a pointer pair.
  void setHLPtr(Register P) {
    mov(FH, f(hi(P)));
    mov(FL, f(lo(P)));
  }
  // H:L <- H:L + 1. Inside a page-bounded object (abi.md sec.4) INL is
  // enough; otherwise carry into H.
  void incHL(bool PageSafe) {
    emit(INR[FL]);
    if (PageSafe)
      return;
    MCSymbol *S = label();
    jccSym(false, CondZ, S);
    emit(INR[FH]);
    bindAfterLast(S);
  }
  // Whether bytes [Off, Off+2) of a memabs address lie in one page-bounded
  // object: our frames and argument areas, or a global of at most 256 bytes.
  bool pageSafe16(const MachineOperand &Base, int64_t Off) const;

  void expandALU(MachineInstr &MI, unsigned Op, bool Imm, bool Wide);
  void expandShift(MachineInstr &MI, unsigned Kind, bool Wide);
  void expandBrCC8(MachineInstr &MI, bool Imm);
  void expandBrCC16(MachineInstr &MI, bool Imm);
  bool expand(MachineInstr &MI);
};

char I8008ExpandPseudos::ID = 0;

} // namespace

INITIALIZE_PASS(I8008ExpandPseudos, DEBUG_TYPE, PASS_NAME, false, false)

bool I8008ExpandPseudos::pageSafe16(const MachineOperand &Base,
                                    int64_t Off) const {
  int64_t O = Base.getOffset() + Off;
  if (Base.isSymbol()) {
    StringRef N = Base.getSymbolName();
    return (N.starts_with("__frame_") || N.starts_with("__args_")) && O >= 0 &&
           O + 2 <= 256;
  }
  if (Base.isGlobal()) {
    const auto *GV = dyn_cast<GlobalVariable>(Base.getGlobal());
    if (!GV || GV->getName().starts_with("__args_"))
      return GV && O >= 0 && O + 2 <= 256;
    uint64_t Size =
        GV->getDataLayout().getTypeAllocSize(GV->getValueType()).getFixedValue();
    return Size <= 256 && O >= 0 && (uint64_t)O + 2 <= Size;
  }
  return false;
}

// ALU pseudos. Op is the I8008Op::Alu of the low byte.
void I8008ExpandPseudos::expandALU(MachineInstr &MI, unsigned Op, bool Imm,
                                   bool Wide) {
  Register D = MI.getOperand(0).getReg(), A = MI.getOperand(1).getReg();
  if (!Wide) {
    if (Imm) {
      int64_t K = MI.getOperand(2).getImm() & 0xff;
      if (Op == SU) {
        Op = AD;
        K = (-K) & 0xff;
      }
      // Identities and INr/DCr (1 byte each, flags are dead).
      if ((Op == AD && K == 0) || (Op == OR && K == 0) ||
          (Op == XR && K == 0) || (Op == ND && K == 0xff)) {
        movR(D, A);
        return;
      }
      if ((Op == ND && K == 0) || (Op == OR && K == 0xff)) {
        mviR(D, K);
        return;
      }
      if (Op == AD && (K == 1 || K == 2 || K == 0xff || K == 0xfe)) {
        movR(D, A);
        unsigned N = (K == 1 || K == 0xff) ? 1 : 2;
        for (unsigned I = 0; I < N; ++I)
          emit(K < 0x80 ? INR[f(D)] : DCR[f(D)]);
        return;
      }
      movR(I8008::A, A); // 4 bytes: la; opi k; ld a
      aluI(Op, K);
      movR(D, I8008::A);
      return;
    }
    Register B = MI.getOperand(2).getReg();
    movR(I8008::A, A); // 3 bytes
    alu(Op, B);
    movR(D, I8008::A);
    return;
  }

  // 16 bits: low byte, then high byte with carry/borrow (AD->AC, SU->SB).
  unsigned HiOp = Op == AD ? AC : Op == SU ? SB : Op;
  if (Imm) {
    int64_t K = MI.getOperand(2).getImm() & 0xffff;
    if (Op == SU) {
      Op = AD;
      HiOp = AC;
      K = (-K) & 0xffff;
    }
    if (Op == AD) {
      if (K == 0) {
        movR(lo(D), lo(A));
        movR(hi(D), hi(A));
        return;
      }
      if (K == 1 && D == A) { // 5 bytes: inr lo; jfz 1f; inr hi; 1:
        emit(INR[f(lo(D))]);
        MCSymbol *S = label();
        jccSym(false, CondZ, S);
        emit(INR[f(hi(D))]);
        bindAfterLast(S);
        return;
      }
      if (K == 0xffff && D == A) { // 7 bytes: la lo; ora; jfz 1f; dc hi; 1: dc lo
        movR(I8008::A, lo(D));
        alu(OR, I8008::A);
        MCSymbol *S = label();
        jccSym(false, CondZ, S);
        emit(DCR[f(hi(D))]);
        emit(DCR[f(lo(D))]);
        std::prev(Pos)->setPreInstrSymbol(*MF, S);
        return;
      }
      if ((K & 0xff) == 0) { // only the high byte changes
        movR(lo(D), lo(A));
        movR(I8008::A, hi(A));
        aluI(AD, K >> 8);
        movR(hi(D), I8008::A);
        return;
      }
      movR(I8008::A, lo(A)); // 8 bytes
      aluI(AD, K);
      movR(lo(D), I8008::A);
      movR(I8008::A, hi(A));
      aluI(AC, K >> 8);
      movR(hi(D), I8008::A);
      return;
    }
    // Logic: per byte, with identities.
    for (int Half = 0; Half < 2; ++Half) {
      Register DS = Half ? hi(D) : lo(D), AS = Half ? hi(A) : lo(A);
      int64_t KB = Half ? (K >> 8) & 0xff : K & 0xff;
      if ((Op == OR || Op == XR) && KB == 0) {
        movR(DS, AS);
      } else if (Op == ND && KB == 0xff) {
        movR(DS, AS);
      } else if ((Op == ND && KB == 0) || (Op == OR && KB == 0xff)) {
        mviR(DS, KB);
      } else {
        movR(I8008::A, AS);
        aluI(Op, KB);
        movR(DS, I8008::A);
      }
    }
    return;
  }
  Register B = MI.getOperand(2).getReg();
  movR(I8008::A, lo(A)); // 6 bytes
  alu(Op, lo(B));
  movR(lo(D), I8008::A);
  movR(I8008::A, hi(A));
  alu(HiOp, hi(B));
  movR(hi(D), I8008::A);
}

// Shifts by a constant. Kind: 0 = shl, 1 = lshr, 2 = ashr.
void I8008ExpandPseudos::expandShift(MachineInstr &MI, unsigned Kind,
                                     bool Wide) {
  Register D = MI.getOperand(0).getReg(), S = MI.getOperand(1).getReg();
  unsigned K = MI.getOperand(2).getImm();
  // One-byte shift of A by N.
  auto shiftA = [&](unsigned N) {
    if (Kind == 2 && N == 7) { // sign fill: rlc; sba
      emit(I8008::RLC);
      alu(SB, I8008::A);
      return;
    }
    for (unsigned I = 0; I < N; ++I) {
      if (Kind == 0) {
        alu(AD, I8008::A); // a + a
      } else if (Kind == 1) {
        emit(I8008::RRC);
      } else { // rlc; rrc puts the sign in carry and restores A; rar shifts it in
        emit(I8008::RLC);
        emit(I8008::RRC);
        emit(I8008::RAR);
      }
    }
    if (Kind == 1 && N)
      aluI(ND, 0xff >> N);
  };

  if (!Wide) {
    movR(I8008::A, S);
    // A long logical shift is a short rotate the other way plus a mask:
    // x >> 7 = rlc; ndi 1 (3 bytes, not 9).
    if (Kind == 1 && K >= 5) {
      for (unsigned I = K; I < 8; ++I)
        emit(I8008::RLC);
      aluI(ND, 0xff >> K);
    } else if (Kind == 0 && K >= 6) {
      for (unsigned I = K; I < 8; ++I)
        emit(I8008::RRC);
      aluI(ND, (0xff << K) & 0xff);
    } else {
      shiftA(K);
    }
    movR(D, I8008::A);
    return;
  }
  if (K >= 8) {
    if (Kind == 0) { // d.hi = s.lo << (k-8); d.lo = 0
      movR(I8008::A, lo(S));
      shiftA(K - 8);
      movR(hi(D), I8008::A);
      mviR(lo(D), 0);
      return;
    }
    movR(I8008::A, hi(S));
    shiftA(K - 8);
    movR(lo(D), I8008::A);
    if (Kind == 1) {
      mviR(hi(D), 0);
    } else {
      movR(I8008::A, hi(S));
      emit(I8008::RLC);
      alu(SB, I8008::A);
      movR(hi(D), I8008::A);
    }
    return;
  }
  Register Src = S;
  for (unsigned I = 0; I < K; ++I, Src = D) {
    if (Kind == 0) { // la lo; ada; llo a; la hi; aca; lhi a
      movR(I8008::A, lo(Src));
      alu(AD, I8008::A);
      movR(lo(D), I8008::A);
      movR(I8008::A, hi(Src));
      alu(AC, I8008::A);
      movR(hi(D), I8008::A);
    } else {
      movR(I8008::A, hi(Src));
      if (Kind == 1) {
        alu(OR, I8008::A); // clears carry
      } else {
        emit(I8008::RLC);
        emit(I8008::RRC);
      }
      emit(I8008::RAR);
      movR(hi(D), I8008::A);
      movR(I8008::A, lo(Src));
      emit(I8008::RAR);
      movR(lo(D), I8008::A);
    }
  }
}

// 8-bit compare and branch: `la a; cp b|cpi k; jcc` (6-7 bytes).
void I8008ExpandPseudos::expandBrCC8(MachineInstr &MI, bool Imm) {
  Register A = MI.getOperand(0).getReg();
  int CC = MI.getOperand(2).getImm();
  const MachineOperand &Dst = MI.getOperand(3);
  bool Signed = CC >= I8008CC::SLT;

  if (Imm) {
    int64_t K = MI.getOperand(1).getImm() & 0xff;
    int64_t Max = Signed ? 0x7f : 0xff;
    int64_t SK = Signed ? (int8_t)K : K; // value in the relation's order
    // a > k == a >= k+1, a <= k == a < k+1.
    if (CC == I8008CC::UGT || CC == I8008CC::SGT || CC == I8008CC::ULE ||
        CC == I8008CC::SLE) {
      bool Gt = CC == I8008CC::UGT || CC == I8008CC::SGT;
      if (SK == Max) { // never / always
        if (!Gt)
          emit(I8008::JMP).add(Dst);
        return;
      }
      K = (K + 1) & 0xff;
      CC = Gt ? (Signed ? I8008CC::SGE : I8008CC::UGE)
              : (Signed ? I8008CC::SLT : I8008CC::ULT);
    }
    movR(I8008::A, A);
    switch (CC) {
    case I8008CC::EQ:
    case I8008CC::NE:
      if (K == 0)
        alu(OR, I8008::A);
      else
        aluI(CP, K);
      jcc(CC == I8008CC::EQ, CondZ, Dst);
      return;
    case I8008CC::ULT:
    case I8008CC::UGE:
      if (K == 0) { // never / always
        std::prev(Pos)->eraseFromParent();
        if (CC == I8008CC::UGE)
          emit(I8008::JMP).add(Dst);
        return;
      }
      aluI(CP, K);
      jcc(CC == I8008CC::ULT, CondC, Dst);
      return;
    default: // SLT / SGE
      if (K == 0) { // sign test
        alu(OR, I8008::A);
        jcc(CC == I8008CC::SLT, CondS, Dst);
        return;
      }
      aluI(XR, 0x80);
      aluI(CP, K ^ 0x80);
      jcc(CC == I8008CC::SLT, CondC, Dst);
      return;
    }
  }

  Register B = MI.getOperand(1).getReg();
  if (CC == I8008CC::UGT || CC == I8008CC::ULE || CC == I8008CC::SGT ||
      CC == I8008CC::SLE) {
    std::swap(A, B);
    CC = I8008CC::swapped(CC);
  }
  if (Signed) { // compare a^0x80 with b^0x80 (b's copy in H)
    movR(I8008::A, B);
    aluI(XR, 0x80);
    movR(I8008::H, I8008::A);
    movR(I8008::A, A);
    aluI(XR, 0x80);
    alu(CP, I8008::H);
  } else {
    movR(I8008::A, A);
    alu(CP, B);
  }
  switch (CC) {
  case I8008CC::EQ: jcc(true, CondZ, Dst); return;
  case I8008CC::NE: jcc(false, CondZ, Dst); return;
  case I8008CC::ULT: case I8008CC::SLT: jcc(true, CondC, Dst); return;
  default: jcc(false, CondC, Dst); return; // UGE / SGE
  }
}

// 16-bit compare and branch.
void I8008ExpandPseudos::expandBrCC16(MachineInstr &MI, bool Imm) {
  Register A = MI.getOperand(0).getReg();
  int CC = MI.getOperand(2).getImm();
  const MachineOperand &Dst = MI.getOperand(3);
  bool Signed = CC >= I8008CC::SLT;

  if (Imm) {
    int64_t K = MI.getOperand(1).getImm() & 0xffff;
    int64_t Max = Signed ? 0x7fff : 0xffff;
    int64_t SK = Signed ? (int16_t)K : K;
    if (CC == I8008CC::UGT || CC == I8008CC::SGT || CC == I8008CC::ULE ||
        CC == I8008CC::SLE) {
      bool Gt = CC == I8008CC::UGT || CC == I8008CC::SGT;
      if (SK == Max) {
        if (!Gt)
          emit(I8008::JMP).add(Dst);
        return;
      }
      K = (K + 1) & 0xffff;
      CC = Gt ? (Signed ? I8008CC::SGE : I8008CC::UGE)
              : (Signed ? I8008CC::SLT : I8008CC::ULT);
    }
    switch (CC) {
    case I8008CC::EQ:
    case I8008CC::NE:
      if (K == 0) { // la lo; or hi; jtz/jfz (5 bytes)
        movR(I8008::A, lo(A));
        alu(OR, hi(A));
        jcc(CC == I8008CC::EQ, CondZ, Dst);
        return;
      }
      movR(I8008::A, lo(A));
      aluI(CP, K);
      if (CC == I8008CC::NE) {
        jcc(false, CondZ, Dst);
        movR(I8008::A, hi(A));
        aluI(CP, K >> 8);
        jcc(false, CondZ, Dst);
      } else {
        MCSymbol *Skip = label();
        jccSym(false, CondZ, Skip);
        movR(I8008::A, hi(A));
        aluI(CP, K >> 8);
        jcc(true, CondZ, Dst);
        bindAfterLast(Skip);
      }
      return;
    case I8008CC::ULT:
    case I8008CC::UGE:
      if (K == 0) {
        if (CC == I8008CC::UGE)
          emit(I8008::JMP).add(Dst);
        return;
      }
      movR(I8008::A, lo(A));
      aluI(SU, K);
      movR(I8008::A, hi(A));
      aluI(SB, K >> 8);
      jcc(CC == I8008CC::ULT, CondC, Dst);
      return;
    default: // SLT / SGE
      if (K == 0) {
        movR(I8008::A, hi(A));
        alu(OR, I8008::A);
        jcc(CC == I8008CC::SLT, CondS, Dst);
        return;
      }
      movR(I8008::A, hi(A));
      aluI(XR, 0x80);
      movR(I8008::H, I8008::A);
      movR(I8008::A, lo(A));
      aluI(SU, K);
      movR(I8008::A, I8008::H);
      aluI(SB, (K >> 8) ^ 0x80);
      jcc(CC == I8008CC::SLT, CondC, Dst);
      return;
    }
  }

  Register B = MI.getOperand(1).getReg();
  if (CC == I8008CC::UGT || CC == I8008CC::ULE || CC == I8008CC::SGT ||
      CC == I8008CC::SLE) {
    std::swap(A, B);
    CC = I8008CC::swapped(CC);
  }
  switch (CC) {
  case I8008CC::EQ: {
    movR(I8008::A, lo(A));
    alu(CP, lo(B));
    MCSymbol *Skip = label();
    jccSym(false, CondZ, Skip);
    movR(I8008::A, hi(A));
    alu(CP, hi(B));
    jcc(true, CondZ, Dst);
    bindAfterLast(Skip);
    return;
  }
  case I8008CC::NE:
    movR(I8008::A, lo(A));
    alu(CP, lo(B));
    jcc(false, CondZ, Dst);
    movR(I8008::A, hi(A));
    alu(CP, hi(B));
    jcc(false, CondZ, Dst);
    return;
  case I8008CC::ULT:
  case I8008CC::UGE:
    movR(I8008::A, lo(A));
    alu(SU, lo(B));
    movR(I8008::A, hi(A));
    alu(SB, hi(B));
    jcc(CC == I8008CC::ULT, CondC, Dst);
    return;
  default: // SLT / SGE: high bytes with the sign flipped, in H (b) and L (a)
    movR(I8008::A, hi(B));
    aluI(XR, 0x80);
    movR(I8008::H, I8008::A);
    movR(I8008::A, hi(A));
    aluI(XR, 0x80);
    movR(I8008::L, I8008::A);
    movR(I8008::A, lo(A));
    alu(SU, lo(B));
    movR(I8008::A, I8008::L);
    alu(SB, I8008::H);
    jcc(CC == I8008CC::SLT, CondC, Dst);
    return;
  }
}

bool I8008ExpandPseudos::expand(MachineInstr &MI) {
  DL = MI.getDebugLoc();
  Pos = MI.getIterator();
  auto Reg = [&](unsigned I) { return MI.getOperand(I).getReg(); };

  switch (MI.getOpcode()) {
  default:
    return false;

  case I8008::MOV8ri:
    mviR(Reg(0), MI.getOperand(1).getImm());
    break;
  case I8008::MOV16ri: {
    int64_t K = MI.getOperand(1).getImm();
    mviR(lo(Reg(0)), K);
    mviR(hi(Reg(0)), K >> 8);
    break;
  }
  case I8008::LEA16: { // l<lo>i %lo(a); l<hi>i %hi(a)
    const MachineOperand &Base = MI.getOperand(1);
    int64_t Off = MI.getOperand(2).getImm();
    emit(MVI[f(lo(Reg(0)))]).add(symOp(Base, Off, I8008II::MO_LO));
    emit(MVI[f(hi(Reg(0)))]).add(symOp(Base, Off, I8008II::MO_HI));
    break;
  }

  case I8008::ADD8rr: expandALU(MI, AD, false, false); break;
  case I8008::SUB8rr: expandALU(MI, SU, false, false); break;
  case I8008::AND8rr: expandALU(MI, ND, false, false); break;
  case I8008::OR8rr:  expandALU(MI, OR, false, false); break;
  case I8008::XOR8rr: expandALU(MI, XR, false, false); break;
  case I8008::ADD8ri: expandALU(MI, AD, true, false); break;
  case I8008::SUB8ri: expandALU(MI, SU, true, false); break;
  case I8008::AND8ri: expandALU(MI, ND, true, false); break;
  case I8008::OR8ri:  expandALU(MI, OR, true, false); break;
  case I8008::XOR8ri: expandALU(MI, XR, true, false); break;
  case I8008::ADD16rr: expandALU(MI, AD, false, true); break;
  case I8008::SUB16rr: expandALU(MI, SU, false, true); break;
  case I8008::AND16rr: expandALU(MI, ND, false, true); break;
  case I8008::OR16rr:  expandALU(MI, OR, false, true); break;
  case I8008::XOR16rr: expandALU(MI, XR, false, true); break;
  case I8008::ADD16ri: expandALU(MI, AD, true, true); break;
  case I8008::SUB16ri: expandALU(MI, SU, true, true); break;
  case I8008::AND16ri: expandALU(MI, ND, true, true); break;
  case I8008::OR16ri:  expandALU(MI, OR, true, true); break;
  case I8008::XOR16ri: expandALU(MI, XR, true, true); break;

  case I8008::RSUB8ri: // lai k; su a; ld a
    emit(MVI[FA]).addImm(MI.getOperand(2).getImm() & 0xff);
    alu(SU, Reg(1));
    movR(Reg(0), I8008::A);
    break;
  case I8008::RSUB16ri: {
    int64_t K = MI.getOperand(2).getImm();
    emit(MVI[FA]).addImm(K & 0xff);
    alu(SU, lo(Reg(1)));
    movR(lo(Reg(0)), I8008::A);
    emit(MVI[FA]).addImm((K >> 8) & 0xff);
    alu(SB, hi(Reg(1)));
    movR(hi(Reg(0)), I8008::A);
    break;
  }

  case I8008::SHL8ri: expandShift(MI, 0, false); break;
  case I8008::SRL8ri: expandShift(MI, 1, false); break;
  case I8008::SRA8ri: expandShift(MI, 2, false); break;
  case I8008::SHL16ri: expandShift(MI, 0, true); break;
  case I8008::SRL16ri: expandShift(MI, 1, true); break;
  case I8008::SRA16ri: expandShift(MI, 2, true); break;

  case I8008::SEXT8: // la a; l<lo>a; rlc; sba; l<hi>a
    movR(I8008::A, Reg(1));
    movR(lo(Reg(0)), I8008::A);
    emit(I8008::RLC);
    alu(SB, I8008::A);
    movR(hi(Reg(0)), I8008::A);
    break;

  case I8008::LD8: // lh ph; ll pl; ld m
    setHLPtr(Reg(1));
    mov(f(Reg(0)), FM);
    break;
  case I8008::LD16:
    setHLPtr(Reg(1));
    mov(f(lo(Reg(0))), FM);
    incHL(false);
    mov(f(hi(Reg(0))), FM);
    break;
  case I8008::ST8:
    setHLPtr(Reg(1));
    mov(FM, f(Reg(0)));
    break;
  case I8008::ST8i:
    setHLPtr(Reg(1));
    emit(MVI[FM]).addImm(MI.getOperand(0).getImm() & 0xff);
    break;
  case I8008::ST16:
    setHLPtr(Reg(1));
    mov(FM, f(lo(Reg(0))));
    incHL(false);
    mov(FM, f(hi(Reg(0))));
    break;

  case I8008::LDA8: // lhi %hi(a); lli %lo(a); ld m
    setHLAbs(MI.getOperand(1), MI.getOperand(2).getImm());
    mov(f(Reg(0)), FM);
    break;
  case I8008::LDA16:
    setHLAbs(MI.getOperand(1), MI.getOperand(2).getImm());
    mov(f(lo(Reg(0))), FM);
    incHL(pageSafe16(MI.getOperand(1), MI.getOperand(2).getImm()));
    mov(f(hi(Reg(0))), FM);
    break;
  case I8008::STA8:
    setHLAbs(MI.getOperand(1), MI.getOperand(2).getImm());
    mov(FM, f(Reg(0)));
    break;
  case I8008::STA8i:
    setHLAbs(MI.getOperand(1), MI.getOperand(2).getImm());
    emit(MVI[FM]).addImm(MI.getOperand(0).getImm() & 0xff);
    break;
  case I8008::STA16:
    setHLAbs(MI.getOperand(1), MI.getOperand(2).getImm());
    mov(FM, f(lo(Reg(0))));
    incHL(pageSafe16(MI.getOperand(1), MI.getOperand(2).getImm()));
    mov(FM, f(hi(Reg(0))));
    break;

  case I8008::CALL: {
    auto Cal = emit(I8008::CAL).add(MI.getOperand(0));
    for (const MachineOperand &MO : MI.operands())
      if (MO.isRegMask())
        Cal.addRegMask(MO.getRegMask());
    break;
  }
  case I8008::CALLind: {
    // Patch the RAM trampoline __icall (`jmp xxxx`, abi.md sec.3.4) and call
    // it. The linker keeps __icall's 3 bytes within one page.
    MachineOperand Tramp = MachineOperand::CreateES("__icall");
    setHLAbs(Tramp, 0);
    emit(INR[FL]);
    mov(FM, f(lo(Reg(0))));
    emit(INR[FL]);
    mov(FM, f(hi(Reg(0))));
    auto Cal = emit(I8008::CAL).addExternalSymbol("__icall");
    for (const MachineOperand &MO : MI.operands())
      if (MO.isRegMask())
        Cal.addRegMask(MO.getRegMask());
    break;
  }
  case I8008::RETpseudo:
    emit(I8008::RET);
    break;
  case I8008::BR:
    emit(I8008::JMP).add(MI.getOperand(0));
    break;
  case I8008::BRCC8rr: expandBrCC8(MI, false); break;
  case I8008::BRCC8ri: expandBrCC8(MI, true); break;
  case I8008::BRCC16rr: expandBrCC16(MI, false); break;
  case I8008::BRCC16ri: expandBrCC16(MI, true); break;
  }
  MI.eraseFromParent();
  return true;
}

bool I8008ExpandPseudos::runOnMachineFunction(MachineFunction &Fn) {
  MF = &Fn;
  const I8008Subtarget &STI = Fn.getSubtarget<I8008Subtarget>();
  TII = STI.getInstrInfo();
  TRI = STI.getRegisterInfo();
  bool Changed = false;
  for (MachineBasicBlock &B : Fn) {
    MBB = &B;
    for (auto I = B.begin(), E = B.end(); I != E;) {
      MachineInstr &MI = *I++;
      Changed |= expand(MI);
    }
  }
  return Changed;
}

FunctionPass *llvm::createI8008ExpandPseudosPass() {
  return new I8008ExpandPseudos();
}
