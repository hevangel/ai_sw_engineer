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
#include "llvm/IR/GlobalValue.h"
#include "llvm/MC/MCContext.h"
#include "llvm/Support/CommandLine.h"
#include <array>

using namespace llvm;

#define DEBUG_TYPE "i4004-expand-pseudos"
#define PASS_NAME "I4004 pseudo-instruction expansion"

// DCL-banked RAM (frozen, issue #18): frames and pointers may live in CM-RAM
// banks 1-3, so every frame access and pointer dereference is bracketed by
// `LD <bank>; DCL ... LDM 0; DCL`. Off by default: all data lives in CM-RAM
// bank 0, DCL is never emitted, and the brackets (~4-6 B per access) vanish.
// The packer then places every frame in bank 0 (it sees no __framebank_ refs).
static cl::opt<bool>
    BankedRAM("i4004-banked-ram",
              cl::desc("I4004: DCL-bracket RAM accesses for frames/pointers in "
                       "CM-RAM banks 1-3 (legacy banked layout)"),
              cl::init(false));

// Software-call convention (frozen, issue #18 C1): compiled functions are
// entered through a 3-nibble token pushed on the 4002 soft stack and return
// via `JUN __soft_ret`, so call depth is unbounded (10 B per call site). Off
// by default: calls are `JMS` and returns `BBL 0` (2 B / 1 B), and the packer
// asserts the static call depth fits the 3-level hardware stack.
static cl::opt<bool>
    SoftCall("i4004-softcall",
             cl::desc("I4004: software-stack calls (JUN __soft_ret returns) "
                      "instead of JMS/BBL"),
             cl::init(false));

/// The legacy soft-call / banked-RAM layouts take no outlined calls (an
/// outlined body could land in another ROM bank); see I4004InstrInfo.cpp.
bool llvm::i4004LegacyLayout() { return SoftCall || BankedRAM; }

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

  // Per-function frame symbol (abi.md 4.4 / brief decision 1). A spilling or
  // frame-local-addressing function F uses RAM chars [__frame_F ..) assigned by
  // the packer's call-graph overlay so that frames live simultaneously on a
  // call path never overlap. Frames live in CM-RAM bank 0 (with the firmware's
  // data + soft stack), so a frame pointer (LEA_FI) dereferences with a plain
  // SRC and needs no DCL bank select -- which could not compose with the
  // firmware's many address-taken frame locals. Set once per function.
  // Both names live in the MCContext (module lifetime): the outliner, a
  // module pass, runs between this pass and the AsmPrinter, and an outlined
  // body may print a frame symbol after its source function is freed, so a
  // name owned by this pass or by the MachineFunction would dangle.
  const char *FrameSym = nullptr; // "__frame_<fn>"

  // DCL-banked frames (DCL frame scheme). A function's frame may live in any
  // CM-RAM bank 0..3; the packer places it and defines __framebank_<fn> as an
  // ABSOLUTE linker symbol whose value is the DCL ACC code for that bank
  // (0->bank0, 1->bank1, 2->bank2, 4->bank3; code 3 = banks 1+2 at once and is
  // never emitted). Because the firmware's soft stack, __cur_bank mirror and
  // every soft-call push/pop helper live in CM-RAM bank 0 and address it with a
  // plain SRC assuming DCL==0, EVERY banked frame access MUST bracket itself
  // with `LD <bank>; DCL` before and `LDM 0; DCL` after, so DCL is always
  // restored to 0 before control can reach a bank-0-assuming helper (including
  // the CALLSOFT prologue push that runs between two spills in this function).
  const char *FrameBankSym = nullptr; // "__framebank_<fn>"

  // Return the hi (sub_hi) and lo (sub_lo) nibble physical registers of a pair.
  std::pair<Register, Register> nibbles(Register Pair) const {
    Register Hi = TRI->getSubReg(Pair, I4004::sub_hi);
    Register Lo = TRI->getSubReg(Pair, I4004::sub_lo);
    return {Hi, Lo};
  }

  // Emit `FIM P0, __framebank_<fn> ; LD r1 ; DCL` -- select this function's
  // frame bank. The IMM8 reloc on __framebank_<fn> lands the DCL code (<=4) in
  // the FIM low byte, so R0(hi)=0, R1(lo)=code; LD R1 loads it into ACC and DCL
  // commits it. Clobbers P0 + ACC (both already in the pseudo's Defs).
  void emitFrameDcl(MachineBasicBlock &MBB, MachineInstr &MI) {
    if (!BankedRAM)
      return;
    const DebugLoc &DL = MI.getDebugLoc();
    Register P0Lo = TRI->getSubReg(I4004::P0, I4004::sub_lo);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
        .addReg(I4004::P0)
        .addExternalSymbol(FrameBankSym, 0);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(P0Lo);
    BuildMI(MBB, MI, DL, TII->get(I4004::DCL));
  }

  // Emit `LDM 0 ; DCL` -- restore DCL to bank 0 (the firmware's canonical bank
  // for the soft stack, data arena and __cur_bank mirror). Clobbers ACC.
  void emitFrameDclReset(MachineBasicBlock &MBB, MachineInstr &MI) {
    if (!BankedRAM)
      return;
    const DebugLoc &DL = MI.getDebugLoc();
    BuildMI(MBB, MI, DL, TII->get(I4004::LDM)).addImm(0);
    BuildMI(MBB, MI, DL, TII->get(I4004::DCL));
  }

  // Load a DCL code value held in a register nibble into ACC and commit it.
  void emitDclFromReg(MachineBasicBlock &MBB, MachineInstr &MI, Register Nib) {
    if (!BankedRAM)
      return;
    const DebugLoc &DL = MI.getDebugLoc();
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Nib);
    BuildMI(MBB, MI, DL, TII->get(I4004::DCL));
  }

  // `__frame_<fn> + Off` as an FIM operand. (MachineInstrBuilder's
  // addExternalSymbol(Name, N) takes TARGET FLAGS as its second argument, not
  // an offset: passing the char offset there silently put every spill, reload
  // and LEA_FI of a function at __frame_<fn>+0, so all slots aliased.)
  MachineOperand frameSymOperand(int64_t Off) const {
    MachineOperand MO = MachineOperand::CreateES(FrameSym);
    MO.setOffset(Off);
    return MO;
  }

  bool expandMI(MachineBasicBlock &MBB, MachineInstr &MI);
  void emitAddSub(MachineBasicBlock &MBB, MachineInstr &MI, unsigned AluOpc);
  void emitAddSub16(MachineBasicBlock &MBB, MachineInstr &MI, unsigned AluOpc);
  void emitCopy(MachineBasicBlock &MBB, MachineInstr &MI);
  void emitBrCC(MachineBasicBlock &MBB, MachineInstr &MI, bool Wide);
  void emitLogicI(MachineBasicBlock &MBB, MachineInstr &MI);
  void emitShiftI(MachineBasicBlock &MBB, MachineInstr &MI);
  void emitNibSel(MachineBasicBlock &MBB, MachineInstr &MI);

  // Register pairs of a PR8 / DR16 value, least significant first.
  SmallVector<Register, 2> pairsLoFirst(Register R) const {
    if (I4004::DR16RegClass.contains(R))
      return {TRI->getSubReg(R, I4004::sub_pair_lo),
              TRI->getSubReg(R, I4004::sub_pair_hi)};
    return {R};
  }
  // Nibble registers of a PR8 / DR16 value, least significant first.
  SmallVector<Register, 4> nibsLoFirst(Register R) const {
    SmallVector<Register, 4> Out;
    for (Register P : pairsLoFirst(R)) {
      Out.push_back(TRI->getSubReg(P, I4004::sub_lo));
      Out.push_back(TRI->getSubReg(P, I4004::sub_hi));
    }
    return Out;
  }
  void emit(MachineBasicBlock &MBB, MachineInstr &MI, unsigned Opc) {
    BuildMI(MBB, MI, MI.getDebugLoc(), TII->get(Opc));
  }
  void emitLD(MachineBasicBlock &MBB, MachineInstr &MI, Register R) {
    BuildMI(MBB, MI, MI.getDebugLoc(), TII->get(I4004::LD)).addReg(R);
  }
  void emitXCH(MachineBasicBlock &MBB, MachineInstr &MI, Register R) {
    BuildMI(MBB, MI, MI.getDebugLoc(), TII->get(I4004::XCH))
        .addReg(R, RegState::Define);
  }
  void emitLDM(MachineBasicBlock &MBB, MachineInstr &MI, unsigned V) {
    BuildMI(MBB, MI, MI.getDebugLoc(), TII->get(I4004::LDM)).addImm(V & 0xF);
  }
  // Dst nibble = Src nibble (nothing when they are the same register).
  void emitMoveNib(MachineBasicBlock &MBB, MachineInstr &MI, Register Dst,
                   Register Src) {
    if (Dst == Src)
      return;
    emitLD(MBB, MI, Src);
    emitXCH(MBB, MI, Dst);
  }
  // ACC = 0xF if bit 3 of nibble register R is set, else 0 (sign fill).
  void emitSignNib(MachineBasicBlock &MBB, MachineInstr &MI, Register R) {
    emitLD(MBB, MI, R);
    emit(MBB, MI, I4004::RAL); // CY = sign
    emit(MBB, MI, I4004::TCC); // ACC = sign (0/1)
    emit(MBB, MI, I4004::DAC); // 1 -> 0, 0 -> 0xF
    emit(MBB, MI, I4004::CMA); // 0 -> 0xF, 0xF -> 0
  }
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
  const int CCImm = MI.getOperand(2).getImm();
  int CC = I4004CC::code(CCImm);
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
  const unsigned N = I4004CC::usedNibbles(CCImm, LN.size());

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

  // Signed: compare (lhs ^ 0x80..) with (rhs ^ 0x80..) unsigned. Flipping bit 3
  // of ACC with `RAL ; CMC ; RAR` leaves CY (the borrow chained in from the
  // lower nibbles) unchanged: RAL parks the old CY in bit 0 and A3 in CY, CMC
  // inverts it, RAR moves it back into bit 3 and restores the old CY. rhs's top
  // nibble is flipped in place and restored after the SUB (both XCHs, LD and
  // the rotates are CY-neutral). lhs == rhs needs no flip: the two cancel.
  const bool Signed = (CC == I4004CC::SLT || CC == I4004CC::SGE);
  const bool FlipSign = Signed && LN[N - 1] != RN[N - 1];
  auto flipAcc3 = [&]() {
    BuildMI(MBB, MI, DL, TII->get(I4004::RAL));
    BuildMI(MBB, MI, DL, TII->get(I4004::CMC));
    BuildMI(MBB, MI, DL, TII->get(I4004::RAR));
  };
  auto flipReg3 = [&](Register R) {
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(R);
    flipAcc3();
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(R, RegState::Define);
  };

  // Relational: full SUB chain, final CY = unsigned (lhs >= rhs). CMC between
  // nibbles (same borrow-polarity fix as emitAddSub).
  BuildMI(MBB, MI, DL, TII->get(I4004::CLC));
  for (unsigned i = 0; i < N; ++i) {
    const bool Top = (i + 1 == N);
    if (Top && FlipSign)
      flipReg3(RN[i]);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(LN[i]);
    if (Top && FlipSign)
      flipAcc3();
    BuildMI(MBB, MI, DL, TII->get(I4004::SUB)).addReg(RN[i]);
    if (!Top)
      BuildMI(MBB, MI, DL, TII->get(I4004::CMC));
  }
  if (FlipSign)
    flipReg3(RN[N - 1]);
  // After the chain CY=1 means no borrow => lhs >= rhs. UGT/ULE and SGT/SLE
  // were canonicalized to ULT/UGE and SLT/SGE by operand swap in mapCC.
  switch (CC) {
  case I4004CC::UGE: case I4004CC::SGE: jcn(0x2, Dest); break; // CY=1
  case I4004CC::ULT: case I4004CC::SLT: jcn(0xA, Dest); break; // CY=0
  default:
    report_fatal_error("I4004: unexpected BRCC relational code after "
                       "canonicalization");
  }
}

// dst = src {AND,OR,XOR} mask, nibble by nibble. Destination nibble i reads
// only source nibble i, so dst == src is safe. Per nibble (m = mask nibble):
//   result constant (AND 0 / OR F / src known zero) -> LDM v ; XCH   (a pair
//                                                      of constants -> FIM)
//   result = src (AND F / OR 0 / XOR 0)            -> LD ; XCH (or nothing)
//   XOR F                                          -> LD ; CMA ; XCH
//   AND 1 / AND 8                                  -> LD ; RAR|RAL ; TCC ...
//   anything else: edit the bits while each sits in CY. ACC+CY form a 5-bit
//   ring under RAL: after the k-th RAL (k = 1..4) CY holds bit 4-k, and the
//   5th RAL restores the nibble. CLC clears (AND), STC sets (OR), CMC flips
//   (XOR) the bit in CY. A lone bit 3 or bit 0 needs only RAL..RAR / RAR..RAL.
void I4004ExpandPseudos::emitLogicI(MachineBasicBlock &MBB, MachineInstr &MI) {
  Register Dst = MI.getOperand(0).getReg();
  Register Src = MI.getOperand(1).getReg();
  const unsigned OpImm = MI.getOperand(2).getImm();
  const uint64_t Mask = MI.getOperand(3).getImm();
  const unsigned LOp = OpImm & 3, KnownZero = (OpImm >> 4) & 0xF;
  auto D = nibsLoFirst(Dst), S = nibsLoFirst(Src);
  auto DP = pairsLoFirst(Dst);

  enum Kind { Const, Copy, Cma, And1, And8, Ring };
  struct Res {
    Kind K;
    unsigned V;   // Const value / Ring bits to edit
  };
  SmallVector<Res, 4> R;
  for (unsigned i = 0; i < D.size(); ++i) {
    unsigned M = (Mask >> (4 * i)) & 0xF;
    if (KnownZero & (1u << i)) {
      R.push_back({Const, LOp == I4004Nib::AND ? 0u : M});
      continue;
    }
    switch (LOp) {
    case I4004Nib::AND:
      R.push_back(M == 0     ? Res{Const, 0}
                  : M == 0xF ? Res{Copy, 0}
                  : M == 1   ? Res{And1, 0}
                  : M == 8   ? Res{And8, 0}
                             : Res{Ring, ~M & 0xF});
      break;
    case I4004Nib::OR:
      R.push_back(M == 0 ? Res{Copy, 0} : M == 0xF ? Res{Const, 0xF}
                                                   : Res{Ring, M});
      break;
    default:
      R.push_back(M == 0 ? Res{Copy, 0} : M == 0xF ? Res{Cma, 0}
                                                   : Res{Ring, M});
      break;
    }
  }
  const unsigned FixOpc = LOp == I4004Nib::AND  ? I4004::CLC
                          : LOp == I4004Nib::OR ? I4004::STC
                                                : I4004::CMC;

  for (unsigned p = 0; p < DP.size(); ++p) {
    const Res &Lo = R[2 * p], &Hi = R[2 * p + 1];
    if (Lo.K == Const && Hi.K == Const) {
      BuildMI(MBB, MI, MI.getDebugLoc(), TII->get(I4004::FIM))
          .addReg(DP[p])
          .addImm((Hi.V << 4) | Lo.V);
      continue;
    }
    for (unsigned i = 2 * p; i < 2 * p + 2; ++i) {
      switch (R[i].K) {
      case Const:
        emitLDM(MBB, MI, R[i].V);
        emitXCH(MBB, MI, D[i]);
        break;
      case Copy:
        emitMoveNib(MBB, MI, D[i], S[i]);
        break;
      case Cma:
        emitLD(MBB, MI, S[i]);
        emit(MBB, MI, I4004::CMA);
        emitXCH(MBB, MI, D[i]);
        break;
      case And1: // ACC = bit0
        emitLD(MBB, MI, S[i]);
        emit(MBB, MI, I4004::RAR);
        emit(MBB, MI, I4004::TCC);
        emitXCH(MBB, MI, D[i]);
        break;
      case And8: // ACC = bit3 << 3: TCC leaves CY=0, two RARs move it up
        emitLD(MBB, MI, S[i]);
        emit(MBB, MI, I4004::RAL);
        emit(MBB, MI, I4004::TCC);
        emit(MBB, MI, I4004::RAR);
        emit(MBB, MI, I4004::RAR);
        emitXCH(MBB, MI, D[i]);
        break;
      case Ring: {
        const unsigned F = R[i].V;
        emitLD(MBB, MI, S[i]);
        if (F == 8) {
          emit(MBB, MI, I4004::RAL);
          emit(MBB, MI, FixOpc);
          emit(MBB, MI, I4004::RAR);
        } else if (F == 1) {
          emit(MBB, MI, I4004::RAR);
          emit(MBB, MI, FixOpc);
          emit(MBB, MI, I4004::RAL);
        } else {
          for (int Bit = 3; Bit >= 0; --Bit) {
            emit(MBB, MI, I4004::RAL);
            if (F & (1u << Bit))
              emit(MBB, MI, FixOpc);
          }
          emit(MBB, MI, I4004::RAL);
        }
        emitXCH(MBB, MI, D[i]);
        break;
      }
      }
    }
  }
}

// dst = src shifted by a constant (0 < amt < width). Q = amt/4 whole nibbles
// are register moves; each of the R = amt%4 remaining bits is one CY chain
// (CLC ; LD ; RAL|RAR ; XCH ...). Moves run in the order that reads each
// source nibble before its register can be overwritten (SHL high->low,
// SRL/SRA low->high), so dst == src is safe; with no nibble move the first
// rotate pass reads src directly. SRA by width-1 is a pure sign fill.
void I4004ExpandPseudos::emitShiftI(MachineBasicBlock &MBB, MachineInstr &MI) {
  Register Dst = MI.getOperand(0).getReg();
  Register Src = MI.getOperand(1).getReg();
  const unsigned Kind = MI.getOperand(2).getImm();
  const unsigned Amt = MI.getOperand(3).getImm();
  auto D = nibsLoFirst(Dst), S = nibsLoFirst(Src);
  const unsigned N = D.size();
  assert(Amt > 0 && Amt < 4 * N && "SHIFTI amount out of range");

  if (Kind == I4004Nib::SRA && Amt == 4 * N - 1) {
    emitSignNib(MBB, MI, S[N - 1]);
    emitXCH(MBB, MI, D[0]);
    for (unsigned i = 1; i < N; ++i)
      emitMoveNib(MBB, MI, D[i], D[i - 1]);
    return;
  }

  const unsigned Q = Amt / 4, R = Amt % 4;
  if (Q) {
    if (Kind == I4004Nib::SHL) {
      for (int i = N - 1; i >= 0; --i) {
        if (i >= (int)Q) {
          emitMoveNib(MBB, MI, D[i], S[i - Q]);
        } else {
          emitLDM(MBB, MI, 0);
          emitXCH(MBB, MI, D[i]);
        }
      }
    } else {
      for (unsigned i = 0; i < N; ++i) {
        if (i + Q < N) {
          emitMoveNib(MBB, MI, D[i], S[i + Q]);
        } else if (Kind == I4004Nib::SRL) {
          emitLDM(MBB, MI, 0);
          emitXCH(MBB, MI, D[i]);
        } else if (i == N - Q) {
          emitSignNib(MBB, MI, D[N - 1 - Q]); // moved top nibble
          emitXCH(MBB, MI, D[i]);
        } else {
          emitMoveNib(MBB, MI, D[i], D[i - 1]);
        }
      }
    }
  }

  // Residual bits on the nibbles that still carry data: SHL works on
  // D[Q..N-1] (the low Q are zero), SRL/SRA on D[0..N-1-Q].
  for (unsigned Pass = 0; Pass < R; ++Pass) {
    const auto &X = (Pass == 0 && Q == 0) ? S : D;
    if (Kind == I4004Nib::SHL) {
      emit(MBB, MI, I4004::CLC);
      for (unsigned i = Q; i < N; ++i) {
        emitLD(MBB, MI, X[i]);
        emit(MBB, MI, I4004::RAL);
        emitXCH(MBB, MI, D[i]);
      }
    } else {
      const int Top = N - 1 - Q;
      if (Kind == I4004Nib::SRL) {
        emit(MBB, MI, I4004::CLC);
      } else {
        emitLD(MBB, MI, X[Top]);
        emit(MBB, MI, I4004::RAL); // CY = sign, shifted back in at the top
      }
      for (int i = Top; i >= 0; --i) {
        emitLD(MBB, MI, X[i]);
        emit(MBB, MI, I4004::RAR);
        emitXCH(MBB, MI, D[i]);
      }
    }
  }
}

// dst nibble i = (sel bit i ? rhs : lhs) nibble i, or 0 where both are known
// zero. Pure moves: or/xor of operands whose set nibbles never overlap.
void I4004ExpandPseudos::emitNibSel(MachineBasicBlock &MBB, MachineInstr &MI) {
  Register Dst = MI.getOperand(0).getReg();
  auto L = nibsLoFirst(MI.getOperand(1).getReg());
  auto R = nibsLoFirst(MI.getOperand(2).getReg());
  const unsigned Sel = MI.getOperand(3).getImm() & 0xF;
  const unsigned Zero = (MI.getOperand(3).getImm() >> 4) & 0xF;
  auto D = nibsLoFirst(Dst);
  auto DP = pairsLoFirst(Dst);
  for (unsigned p = 0; p < DP.size(); ++p) {
    if (((Zero >> (2 * p)) & 3) == 3) {
      BuildMI(MBB, MI, MI.getDebugLoc(), TII->get(I4004::FIM))
          .addReg(DP[p])
          .addImm(0);
      continue;
    }
    for (unsigned i = 2 * p; i < 2 * p + 2; ++i) {
      if (Zero & (1u << i)) {
        emitLDM(MBB, MI, 0);
        emitXCH(MBB, MI, D[i]);
      } else {
        emitMoveNib(MBB, MI, D[i], (Sel & (1u << i)) ? R[i] : L[i]);
      }
    }
  }
}

bool I4004ExpandPseudos::expandMI(MachineBasicBlock &MBB, MachineInstr &MI) {
  const DebugLoc &DL = MI.getDebugLoc();
  switch (MI.getOpcode()) {
  default:
    return false;

  case I4004::RETpseudo:
    if (!SoftCall) {
      // Hardware stack (abi.md 4.4, issue #18 C1): BBL 0.
      BuildMI(MBB, MI, DL, TII->get(I4004::BBL)).addImm(0);
      MI.eraseFromParent();
      return true;
    }
    // Software-return-stack return (abi.md 4.4.1): a compiled function is
    // entered via the soft-call convention (the caller pushed a 3-nibble return
    // token and JUN'd here, bypassing the hardware stack), so it returns by
    // JUN __soft_ret, which pops the token and dispatches. BBL is reserved for
    // the hand-written leaf rt/HAL helpers (asm, JMS/BBL, depth <= 3).
    BuildMI(MBB, MI, DL, TII->get(I4004::JUN))
        .addExternalSymbol("__soft_ret");
    MI.eraseFromParent();
    return true;

  case I4004::CALL: {
    // A call to a hand-written LEAF runtime/HAL helper (libi4004rt: __mulqi3,
    // __andhi3, memcpy, ... -- external symbols) keeps the hardware stack:
    // those helpers return via BBL and their own call graph is <= 3 deep
    // (asserted by the packer, abi.md 4.4.2). Everything else -- a call to a
    // compiled firmware function, which returns via __soft_ret -- uses the
    // software-call convention.
    const MachineOperand &Callee0 = MI.getOperand(0);
    bool IsLeafHelper = false;
    StringRef Name;
    if (Callee0.isSymbol())
      Name = Callee0.getSymbolName();
    else if (Callee0.isGlobal())
      Name = Callee0.getGlobal()->getName();
    if (!Name.empty())
      IsLeafHelper = Name.starts_with("__") || Name == "memcpy" ||
                     Name == "memset" || Name == "memmove";
    if (IsLeafHelper || !SoftCall) {
      BuildMI(MBB, MI, DL, TII->get(I4004::JMS)).add(MI.getOperand(0));
      MI.eraseFromParent();
      return true;
    }
    // Software call (abi.md 4.4.1), expanded post-RA. The 3-nibble return token
    // is pushed through the scratch pair P0 ONLY (no argument register --
    // P1/D1=P2:P3/D2=P4:P5 -- is touched, so i16 args in D1/D2 survive), by two
    // leaf helpers that each advance the SSP (P7) as a full 8-bit pointer so
    // depth is bounded by the whole 4002 arena, not one 16-char register:
    //   fim p0, %lo8(.Lret)   ; R0 = off.hi, R1 = off.lo
    //   jms __push_off        ; push off.lo, off.hi ; SSP += 2 ; bbl
    //   fim p0, %page(.Lret)  ; R0 = page (byte hi nibble)
    //   jms __push_pg         ; push page ; SSP += 1 ; bbl
    //   jun <callee>          ; NOT jms -- hardware stack bypassed
    // .Lret:
    // __push_off/__push_pg are leaves (hardware depth 1).
    MachineFunction &MF = *MBB.getParent();
    MachineBasicBlock *RetBB = MF.CreateMachineBasicBlock(MBB.getBasicBlock());
    MF.insert(std::next(MBB.getIterator()), RetBB);
    RetBB->splice(RetBB->begin(), &MBB, std::next(MI.getIterator()), MBB.end());
    RetBB->transferSuccessorsAndUpdatePHIs(&MBB);
    MBB.addSuccessor(RetBB);
    RetBB->setLabelMustBeEmitted();

    MachineOperand Callee = MI.getOperand(0);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
        .addReg(I4004::P0)
        .addMBB(RetBB, I4004II::MO_LO8);
    BuildMI(MBB, MI, DL, TII->get(I4004::JMS)).addExternalSymbol("__push_off");
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
        .addReg(I4004::P0)
        .addMBB(RetBB, I4004II::MO_PAGE);
    BuildMI(MBB, MI, DL, TII->get(I4004::JMS)).addExternalSymbol("__push_pg");
    BuildMI(MBB, MI, DL, TII->get(I4004::JUN)).add(Callee);
    MI.eraseFromParent();
    return true;
  }

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
    // dst(DR16) = RAM address of a frame slot (DCL frame scheme). Operand1 is
    // the WITHIN-FRAME char offset (set by eliminateFrameIndex); the low pair
    // is __frame_<fn> + off (R_I4004_IMM8, packer-assigned per-function base,
    // a 0..0xFF char index WITHIN this function's CM-RAM bank). The frame may
    // live in CM-RAM bank 0..3, so the HIGH pair carries the DCL bank code in
    // its low nibble (R_I4004_IMM8 on __framebank_<fn>, value <=4), high nibble
    // 0. A deref of this pointer (LDRAMi8/STRAMi8/LDRAMi16/STRAMi16) reads
    // ptr.hi.lo, DCL-selects that bank, accesses, then restores DCL 0.
    Register Dst = MI.getOperand(0).getReg();
    int64_t Off = MI.getOperand(1).getImm() & 0xFF;
    Register LoP = TRI->getSubReg(Dst, I4004::sub_pair_lo);
    Register HiP = TRI->getSubReg(Dst, I4004::sub_pair_hi);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
        .addReg(LoP)
        .add(frameSymOperand(Off));
    // high pair = DCL bank code (low nibble), high nibble 0; always bank 0
    // without -i4004-banked-ram.
    if (BankedRAM)
      BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
          .addReg(HiP)
          .addExternalSymbol(FrameBankSym, 0);
    else
      BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(HiP).addImm(0);
    MI.eraseFromParent();
    return true;
  }

  case I4004::LEA_GA: {
    // dst(DR16): low pair = FIM sym (R_I4004_IMM8 low byte), high pair = 0.
    // Backbone limitation: the symbol must lie in the low 256 bytes (the high
    // byte is not relocated); a %hi reloc + linker lands with milestone 5.
    Register Dst = MI.getOperand(0).getReg();
    const MachineOperand &Sym = MI.getOperand(1);
    Register LoP = TRI->getSubReg(Dst, I4004::sub_pair_lo);
    Register HiP = TRI->getSubReg(Dst, I4004::sub_pair_hi);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM)).addReg(LoP).add(Sym);
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
    // SRC address byte, whose low nibble (sub_lo) is the char index. Its HIGH
    // sub-pair's low nibble (sub_pair_hi.sub_lo) is the DCL bank code (DCL frame
    // scheme): a frame pointer carries its CM-RAM bank there; a bank-0 pointer
    // (global, firmware data, LEA_GA) carries 0, so the DCL select is a
    // harmless re-select of bank 0. Bracket: LD bank;DCL ... LDM 0;DCL so DCL
    // is restored to 0 before any bank-0-assuming soft-call helper can run.
    Register Dst = MI.getOperand(0).getReg();
    Register Ptr = MI.getOperand(1).getReg();
    Register LoPair = TRI->getSubReg(Ptr, I4004::sub_pair_lo);
    Register HiPair = TRI->getSubReg(Ptr, I4004::sub_pair_hi);
    Register BankReg = TRI->getSubReg(HiPair, I4004::sub_lo);
    Register CharReg = TRI->getSubReg(LoPair, I4004::sub_lo);
    auto [DHi, DLo] = nibbles(Dst);
    // The destination may share the address pair (the pointer dies here and
    // the pseudo has no early-clobber): writing the low nibble would then move
    // the char index before the second SRC. Walk a copy in scratch P0 instead.
    const bool Overlap = TRI->regsOverlap(Dst, LoPair);
    const bool Killed = MI.getOperand(1).isKill();
    emitDclFromReg(MBB, MI, BankReg);
    Register AddrPair = LoPair, AddrChar = CharReg;
    if (Overlap) {
      auto [PHi, PLo] = nibbles(LoPair);
      auto [ZHi, ZLo] = nibbles(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PHi);
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(ZHi, RegState::Define);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PLo);
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(ZLo, RegState::Define);
      AddrPair = I4004::P0;
      AddrChar = ZLo;
    }
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(AddrPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DLo, RegState::Define);
    BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(AddrChar);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(AddrPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DHi, RegState::Define);
    // Restore the char index (DAC = ACC-1) so the pointer is unchanged --
    // unless the pointer dies here (or was never touched).
    if (!Overlap && !Killed) {
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(CharReg);
      BuildMI(MBB, MI, DL, TII->get(I4004::DAC));
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH))
          .addReg(CharReg, RegState::Define);
    }
    emitFrameDclReset(MBB, MI);
    MI.eraseFromParent();
    return true;
  }

  case I4004::STRAMi8: {
    // RAM[ptr] = val(i8). Same addressing + DCL bracket as LDRAMi8.
    Register Val = MI.getOperand(0).getReg();
    Register Ptr = MI.getOperand(1).getReg();
    Register LoPair = TRI->getSubReg(Ptr, I4004::sub_pair_lo);
    Register HiPair = TRI->getSubReg(Ptr, I4004::sub_pair_hi);
    Register BankReg = TRI->getSubReg(HiPair, I4004::sub_lo);
    Register CharReg = TRI->getSubReg(LoPair, I4004::sub_lo);
    auto [VHi, VLo] = nibbles(Val);
    emitDclFromReg(MBB, MI, BankReg);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(LoPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(VLo);
    BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
    BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(CharReg);
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(LoPair);
    BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(VHi);
    BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
    // Restore the char index unless both the pointer and the value die here
    // (the value may share the address pair; then it must be restored too).
    if (!(MI.getOperand(1).isKill() &&
          (MI.getOperand(0).isKill() || !TRI->regsOverlap(Val, LoPair)))) {
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(CharReg);
      BuildMI(MBB, MI, DL, TII->get(I4004::DAC));
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH))
          .addReg(CharReg, RegState::Define);
    }
    emitFrameDclReset(MBB, MI);
    MI.eraseFromParent();
    return true;
  }

  case I4004::LDRAMi16:
  case I4004::STRAMi16: {
    // i16 RAM access: four consecutive chars, low byte first. Copy the pointer
    // byte into P0 so we can INC it 0..3 without disturbing the pointer, then
    // SRC P0 / RDM|WRM each char. Value halves: low pair = chars 0,1; high pair
    // = chars 2,3 (nibble order low,hi within each byte). DCL frame scheme: the
    // pointer's HIGH sub-pair low nibble is the CM-RAM bank code -- select it
    // before the access and restore DCL 0 after (bank-0 pointers carry 0, a
    // harmless re-select). The bank read happens BEFORE P0 is overwritten.
    const bool IsLoad = (MI.getOpcode() == I4004::LDRAMi16);
    Register ValDR = MI.getOperand(0).getReg();
    Register Ptr = MI.getOperand(1).getReg();
    Register PtrLoPair = TRI->getSubReg(Ptr, I4004::sub_pair_lo);
    Register PtrHiPair = TRI->getSubReg(Ptr, I4004::sub_pair_hi);
    Register BankReg = TRI->getSubReg(PtrHiPair, I4004::sub_lo);
    // DCL-select the pointer's bank (reads BankReg; clobbers ACC -- a Def).
    emitDclFromReg(MBB, MI, BankReg);
    // P0 = copy of the pointer's low byte (address), via LD/XCH nibble moves --
    // or, when the pointer dies here and the loaded value does not land on
    // its address pair, walk the pointer's own char index (saves 4 B).
    Register WalkPair = I4004::P0;
    Register P0Lo = TRI->getSubReg(I4004::P0, I4004::sub_lo);
    Register PHi = TRI->getSubReg(PtrLoPair, I4004::sub_hi);
    Register PLo = TRI->getSubReg(PtrLoPair, I4004::sub_lo);
    if (MI.getOperand(1).isKill() && !TRI->regsOverlap(ValDR, PtrLoPair)) {
      WalkPair = PtrLoPair;
      P0Lo = PLo;
    } else {
      Register P0Hi = TRI->getSubReg(I4004::P0, I4004::sub_hi);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PHi);
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(P0Hi, RegState::Define);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(PLo);
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(P0Lo, RegState::Define);
    }
    Register ValLoPair = TRI->getSubReg(ValDR, I4004::sub_pair_lo);
    Register ValHiPair = TRI->getSubReg(ValDR, I4004::sub_pair_hi);
    Register Nib[4] = {TRI->getSubReg(ValLoPair, I4004::sub_lo),
                       TRI->getSubReg(ValLoPair, I4004::sub_hi),
                       TRI->getSubReg(ValHiPair, I4004::sub_lo),
                       TRI->getSubReg(ValHiPair, I4004::sub_hi)};
    for (int i = 0; i < 4; ++i) {
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(WalkPair);
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
    emitFrameDclReset(MBB, MI);
    MI.eraseFromParent();
    return true;
  }

  case I4004::ADDi8ri:
  case I4004::ADDi16ri: {
    // dst = src + imm, nibble by nibble with the constant through LDM:
    // nibbles below the lowest nonzero immediate nibble are copies (no
    // carry); then CLC; LDM c; ADD s; XCH d, and above it each nibble adds
    // the carry (TCC; ADD s when its immediate nibble is 0). `+1` uses IAC.
    auto D = nibsLoFirst(MI.getOperand(0).getReg());
    auto S = nibsLoFirst(MI.getOperand(1).getReg());
    const uint64_t Imm = MI.getOperand(2).getImm() &
                         (MI.getOpcode() == I4004::ADDi16ri ? 0xFFFF : 0xFF);
    unsigned K = 0;
    while (K < D.size() && ((Imm >> (4 * K)) & 0xF) == 0) {
      emitMoveNib(MBB, MI, D[K], S[K]);
      ++K;
    }
    for (unsigned i = K; i < D.size(); ++i) {
      unsigned C = (Imm >> (4 * i)) & 0xF;
      if (i == K) {
        if (C == 1) {
          emitLD(MBB, MI, S[i]);
          emit(MBB, MI, I4004::IAC); // CY = carry out
        } else {
          emit(MBB, MI, I4004::CLC);
          emitLDM(MBB, MI, C);
          BuildMI(MBB, MI, DL, TII->get(I4004::ADD)).addReg(S[i]);
        }
      } else if (C == 0) {
        emit(MBB, MI, I4004::TCC); // ACC = carry, CY = 0
        BuildMI(MBB, MI, DL, TII->get(I4004::ADD)).addReg(S[i]);
      } else {
        emitLDM(MBB, MI, C);
        BuildMI(MBB, MI, DL, TII->get(I4004::ADD)).addReg(S[i]);
      }
      emitXCH(MBB, MI, D[i]);
    }
    MI.eraseFromParent();
    return true;
  }

  case I4004::BRCCi8ri:
  case I4004::BRCCi16ri: {
    // Compare x with an immediate. EQ/NE: per nibble CLC; LDM c; SUB x;
    // JCN Z|NZ (c - x == 0 iff equal). ULE/UGT: the c - x borrow chain, CY=1
    // iff x <= c. NEG/NONNEG: the sign bit via LD top; RAL.
    auto X = nibsLoFirst(MI.getOperand(0).getReg());
    const uint64_t Imm = MI.getOperand(1).getImm();
    const int CCImm = MI.getOperand(2).getImm();
    const int CC = I4004CC::code(CCImm);
    MachineBasicBlock *Dest = MI.getOperand(3).getMBB();
    const unsigned N = I4004CC::usedNibbles(CCImm, X.size());
    auto jcn = [&](int Cond, MachineBasicBlock *T) {
      BuildMI(MBB, MI, DL, TII->get(I4004::JCN)).addImm(Cond).addMBB(T);
    };
    if (CC == I4004CC::NEG || CC == I4004CC::NONNEG) {
      emitLD(MBB, MI, X[N - 1]);
      emit(MBB, MI, I4004::RAL);
      jcn(CC == I4004CC::NEG ? 0x2 : 0xA, Dest);
    } else if (CC == I4004CC::EQ || CC == I4004CC::NE) {
      MachineBasicBlock *FallThru = nullptr;
      for (MachineBasicBlock *Succ : MBB.successors())
        if (Succ != Dest)
          FallThru = Succ;
      Dest->setLabelMustBeEmitted();
      if (FallThru)
        FallThru->setLabelMustBeEmitted();
      for (unsigned i = 0; i < N; ++i) {
        const unsigned C = (Imm >> (4 * i)) & 0xF;
        if (C == 0) {
          // x == 0 iff ACC == 0 after LD x: 1 B instead of CLC; LDM 0; SUB x.
          emitLD(MBB, MI, X[i]);
        } else {
          emit(MBB, MI, I4004::CLC);
          emitLDM(MBB, MI, C);
          BuildMI(MBB, MI, DL, TII->get(I4004::SUB)).addReg(X[i]);
        }
        if (CC == I4004CC::NE)
          jcn(0xC, Dest);
        else if (i + 1 < N) {
          if (FallThru)
            jcn(0xC, FallThru);
        } else
          jcn(0x4, Dest);
      }
    } else {
      emit(MBB, MI, I4004::CLC);
      for (unsigned i = 0; i < N; ++i) {
        emitLDM(MBB, MI, (Imm >> (4 * i)) & 0xF);
        BuildMI(MBB, MI, DL, TII->get(I4004::SUB)).addReg(X[i]);
        if (i + 1 < N)
          emit(MBB, MI, I4004::CMC);
      }
      if (CC == I4004CC::ULE)
        jcn(0x2, Dest); // CY=1: c >= x
      else if (CC == I4004CC::UGT)
        jcn(0xA, Dest); // CY=0: c < x
      else
        report_fatal_error("I4004: bad BRCC*ri condition");
    }
    MI.eraseFromParent();
    return true;
  }

  case I4004::BRRAM: {
    // SRC a; RDM|RDk; [XCH R1; LDM c; CLC; SUB R1;] JCN Z|NZ dest.
    static const unsigned RdK[5] = {I4004::RD0, I4004::RD1, I4004::RD2,
                                    I4004::RD3, I4004::RDM};
    const int64_t KC = MI.getOperand(1).getImm();
    const unsigned K = KC & 0xF, C = (KC >> 4) & 0xF;
    const int CC = I4004CC::code(MI.getOperand(2).getImm());
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(MI.getOperand(0).getReg());
    BuildMI(MBB, MI, DL, TII->get(RdK[K]));
    if (C != 0) {
      Register R1 = TRI->getSubReg(I4004::P0, I4004::sub_lo);
      emitXCH(MBB, MI, R1);
      emitLDM(MBB, MI, C);
      emit(MBB, MI, I4004::CLC);
      BuildMI(MBB, MI, DL, TII->get(I4004::SUB)).addReg(R1);
    }
    BuildMI(MBB, MI, DL, TII->get(I4004::JCN))
        .addImm(CC == I4004CC::EQ ? 0x4 : 0xC)
        .add(MI.getOperand(3));
    MI.eraseFromParent();
    return true;
  }

  case I4004::RAMRD:
  case I4004::RAMRS: {
    // dst = 4002 main character / status character k at the SRC address.
    static const unsigned RdK[4] = {I4004::RD0, I4004::RD1, I4004::RD2,
                                    I4004::RD3};
    Register Dst = MI.getOperand(0).getReg();
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(MI.getOperand(1).getReg());
    BuildMI(MBB, MI, DL,
            TII->get(MI.getOpcode() == I4004::RAMRD
                         ? I4004::RDM
                         : RdK[MI.getOperand(2).getImm() & 3]));
    auto [DHi, DLo] = nibbles(Dst);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DLo, RegState::Define);
    BuildMI(MBB, MI, DL, TII->get(I4004::LDM)).addImm(0);
    BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(DHi, RegState::Define);
    MI.eraseFromParent();
    return true;
  }

  case I4004::RAMWR:
  case I4004::RAMWS:
  case I4004::RAMWRi:
  case I4004::RAMWSi: {
    static const unsigned WrK[4] = {I4004::WR0, I4004::WR1, I4004::WR2,
                                    I4004::WR3};
    const unsigned Opc = MI.getOpcode();
    BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(MI.getOperand(0).getReg());
    if (Opc == I4004::RAMWRi || Opc == I4004::RAMWSi) {
      emitLDM(MBB, MI, MI.getOperand(1).getImm());
    } else {
      auto [VHi, VLo] = nibbles(MI.getOperand(1).getReg());
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(VLo);
    }
    BuildMI(MBB, MI, DL,
            TII->get(Opc == I4004::RAMWR || Opc == I4004::RAMWRi
                         ? I4004::WRM
                         : WrK[MI.getOperand(2).getImm() & 3]));
    MI.eraseFromParent();
    return true;
  }

  case I4004::LOGICIi8:
  case I4004::LOGICIi16:
    emitLogicI(MBB, MI);
    MI.eraseFromParent();
    return true;

  case I4004::SHIFTIi8:
  case I4004::SHIFTIi16:
    emitShiftI(MBB, MI);
    MI.eraseFromParent();
    return true;

  case I4004::NIBSELi8:
  case I4004::NIBSELi16:
    emitNibSel(MBB, MI);
    MI.eraseFromParent();
    return true;

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
    // RAM[frame + off] = src, in this function's per-function frame. Operand1 is
    // the WITHIN-FRAME char offset (set by eliminateFrameIndex); the base char
    // comes from the external symbol __frame_<fn> (packer-assigned by a
    // call-graph overlay) so a callee never clobbers a caller's spills. DCL
    // frame scheme: the frame may live in CM-RAM bank 0..3; __framebank_<fn> is
    // its DCL code. Bracket the access: select the frame bank, SRC/WRM, then
    // restore DCL 0 so the soft stack / soft-call helpers (bank 0, plain SRC)
    // and any CALLSOFT push between two spills in this function stay correct.
    Register Src = MI.getOperand(0).getReg();
    int64_t Off = MI.getOperand(1).getImm() & 0xFF;
    const bool Wide = (MI.getOpcode() == I4004::SPILL16);
    SmallVector<Register, 2> Pairs;
    if (Wide) {
      Pairs.push_back(TRI->getSubReg(Src, I4004::sub_pair_lo));
      Pairs.push_back(TRI->getSubReg(Src, I4004::sub_pair_hi));
    } else {
      Pairs.push_back(Src);
    }
    Register P0Lo = TRI->getSubReg(I4004::P0, I4004::sub_lo);
    SmallVector<Register, 4> Nibs;
    for (Register P : Pairs) {
      Nibs.push_back(TRI->getSubReg(P, I4004::sub_lo));
      Nibs.push_back(TRI->getSubReg(P, I4004::sub_hi));
    }
    // Select the frame bank FIRST (clobbers P0 + ACC), then load the address.
    emitFrameDcl(MBB, MI);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
        .addReg(I4004::P0)
        .add(frameSymOperand(Off));
    for (unsigned i = 0; i < Nibs.size(); ++i) {
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::LD)).addReg(Nibs[i]);
      BuildMI(MBB, MI, DL, TII->get(I4004::WRM));
      if (i + 1 < Nibs.size())
        BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(P0Lo);
    }
    emitFrameDclReset(MBB, MI);
    MI.eraseFromParent();
    return true;
  }

  case I4004::RELOAD8:
  case I4004::RELOAD16: {
    // dst = RAM[frame + off], this function's per-function frame. DCL frame
    // scheme: select __framebank_<fn> before the read, restore DCL 0 after.
    Register Dst = MI.getOperand(0).getReg();
    int64_t Off = MI.getOperand(1).getImm() & 0xFF;
    const bool Wide = (MI.getOpcode() == I4004::RELOAD16);
    SmallVector<Register, 2> Pairs;
    if (Wide) {
      Pairs.push_back(TRI->getSubReg(Dst, I4004::sub_pair_lo));
      Pairs.push_back(TRI->getSubReg(Dst, I4004::sub_pair_hi));
    } else {
      Pairs.push_back(Dst);
    }
    Register P0Lo = TRI->getSubReg(I4004::P0, I4004::sub_lo);
    SmallVector<Register, 4> Nibs;
    for (Register P : Pairs) {
      Nibs.push_back(TRI->getSubReg(P, I4004::sub_lo));
      Nibs.push_back(TRI->getSubReg(P, I4004::sub_hi));
    }
    emitFrameDcl(MBB, MI);
    BuildMI(MBB, MI, DL, TII->get(I4004::FIM))
        .addReg(I4004::P0)
        .add(frameSymOperand(Off));
    for (unsigned i = 0; i < Nibs.size(); ++i) {
      BuildMI(MBB, MI, DL, TII->get(I4004::SRC)).addReg(I4004::P0);
      BuildMI(MBB, MI, DL, TII->get(I4004::RDM));
      BuildMI(MBB, MI, DL, TII->get(I4004::XCH)).addReg(Nibs[i], RegState::Define);
      if (i + 1 < Nibs.size())
        BuildMI(MBB, MI, DL, TII->get(I4004::INC)).addReg(P0Lo);
    }
    emitFrameDclReset(MBB, MI);
    MI.eraseFromParent();
    return true;
  }
  }
}

bool I4004ExpandPseudos::runOnMachineFunction(MachineFunction &MF) {
  const I4004Subtarget &STI = MF.getSubtarget<I4004Subtarget>();
  TII = STI.getInstrInfo();
  TRI = STI.getRegisterInfo();

  // Per-function frame symbol (brief decision 1). The packer defines
  // __frame_<fn> (RAM char base, bank 0) for every function that uses a frame;
  // a function with no frame access emits no reference, so the packer never
  // allocates one for it.
  auto intern = [&MF](const Twine &T) {
    std::string S = T.str();
    char *P = static_cast<char *>(MF.getContext().allocate(S.size() + 1, 1));
    memcpy(P, S.c_str(), S.size() + 1);
    return static_cast<const char *>(P);
  };
  FrameSym = intern("__frame_" + MF.getName());
  FrameBankSym = intern("__framebank_" + MF.getName());

  bool Changed = false;

  // CALL expansion can SPLIT a basic block (the soft-call return site becomes a
  // new MBB), which invalidates a range-for over the function and the per-block
  // iterator. So collect the calls first, then expand each -- exactly the
  // pattern I4004BranchRelax uses for the same reason.
  SmallVector<MachineInstr *, 16> Calls;
  for (MachineBasicBlock &MBB : MF)
    for (MachineInstr &MI : MBB)
      if (MI.getOpcode() == I4004::CALL)
        Calls.push_back(&MI);
  for (MachineInstr *MI : Calls)
    Changed |= expandMI(*MI->getParent(), *MI);

  // Now expand the remaining (non-splitting) pseudos. New MBBs created above are
  // included by this walk.
  for (MachineBasicBlock &MBB : MF) {
    for (auto I = MBB.begin(), E = MBB.end(); I != E;) {
      MachineInstr &MI = *I++;
      if (MI.getOpcode() == I4004::CALL)
        continue; // already handled
      Changed |= expandMI(MBB, MI);
    }
  }
  return Changed;
}

FunctionPass *llvm::createI4004ExpandPseudosPass() {
  return new I4004ExpandPseudos();
}
