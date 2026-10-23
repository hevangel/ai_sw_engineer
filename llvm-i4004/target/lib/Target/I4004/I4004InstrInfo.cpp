//===-- I4004InstrInfo.cpp - I4004 Instruction Information ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004InstrInfo.h"
#include "I4004.h"
#include "I4004ISelLowering.h"
#include "I4004Subtarget.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include "llvm/IR/Module.h"

using namespace llvm;

#define GET_INSTRINFO_CTOR_DTOR
#include "I4004GenInstrInfo.inc"

void I4004InstrInfo::anchor() {}

I4004InstrInfo::I4004InstrInfo(const I4004Subtarget &STI)
    : I4004GenInstrInfo(STI, RI, I4004::ADJCALLSTACKDOWN, I4004::ADJCALLSTACKUP),
      RI() {}

void I4004InstrInfo::copyPhysReg(MachineBasicBlock &MBB,
                                 MachineBasicBlock::iterator MI,
                                 const DebugLoc &DL, Register DestReg,
                                 Register SrcReg, bool KillSrc,
                                 bool RenamableDest, bool RenamableSrc) const {
  // Emit the real nibble moves directly (LD src; XCH dst, per nibble). We do
  // NOT use a pseudo here because copyPhysReg can be invoked by passes that run
  // after I4004ExpandPseudos (e.g. post-RA copy lowering), which would leave an
  // unexpanded pseudo for the AsmPrinter.
  auto copyPair = [&](Register D, Register S) {
    Register DHi = RI.getSubReg(D, I4004::sub_hi);
    Register DLo = RI.getSubReg(D, I4004::sub_lo);
    Register SHi = RI.getSubReg(S, I4004::sub_hi);
    Register SLo = RI.getSubReg(S, I4004::sub_lo);
    BuildMI(MBB, MI, DL, get(I4004::LD)).addReg(SHi);
    BuildMI(MBB, MI, DL, get(I4004::XCH)).addReg(DHi, RegState::Define);
    BuildMI(MBB, MI, DL, get(I4004::LD)).addReg(SLo);
    BuildMI(MBB, MI, DL, get(I4004::XCH)).addReg(DLo, RegState::Define);
  };

  if (I4004::DR16RegClass.contains(DestReg, SrcReg)) {
    copyPair(RI.getSubReg(DestReg, I4004::sub_pair_lo),
             RI.getSubReg(SrcReg, I4004::sub_pair_lo));
    copyPair(RI.getSubReg(DestReg, I4004::sub_pair_hi),
             RI.getSubReg(SrcReg, I4004::sub_pair_hi));
    return;
  }
  copyPair(DestReg, SrcReg);
}

void I4004InstrInfo::storeRegToStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register SrcReg,
    bool isKill, int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    MachineInstr::MIFlag Flags) const {
  // Spill to the 4002-RAM spill region (abi.md sec.2.1). The frame index is
  // carried on a SPILL pseudo and turned into a concrete RAM address by the
  // post-RA expander (I4004ExpandPseudos). DR16 spills as two i8 halves.
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  if (I4004::DR16RegClass.hasSubClassEq(RC)) {
    BuildMI(MBB, MI, DL, get(I4004::SPILL16))
        .addReg(SrcReg, getKillRegState(isKill))
        .addFrameIndex(FrameIndex);
  } else {
    BuildMI(MBB, MI, DL, get(I4004::SPILL8))
        .addReg(SrcReg, getKillRegState(isKill))
        .addFrameIndex(FrameIndex);
  }
}

void I4004InstrInfo::loadRegFromStackSlot(
    MachineBasicBlock &MBB, MachineBasicBlock::iterator MI, Register DestReg,
    int FrameIndex, const TargetRegisterClass *RC, Register VReg,
    unsigned SubReg, MachineInstr::MIFlag Flags) const {
  DebugLoc DL = MI != MBB.end() ? MI->getDebugLoc() : DebugLoc();
  if (I4004::DR16RegClass.hasSubClassEq(RC)) {
    BuildMI(MBB, MI, DL, get(I4004::RELOAD16), DestReg)
        .addFrameIndex(FrameIndex);
  } else {
    BuildMI(MBB, MI, DL, get(I4004::RELOAD8), DestReg).addFrameIndex(FrameIndex);
  }
}

//===----------------------------------------------------------------------===//
// Branch analysis / editing.
//
// The target's terminators are:
//   BR <mbb>                  - unconditional (-> JUN), analyzable + editable.
//   BRCCi8/BRCCi16 ...,<mbb>   - conditional compare-and-branch (-> SUB + JCN).
//   BRZ/BRNZ ...,<mbb>         - conditional zero-test branch.
// Only BR is representable as a generic (TBB, FBB, Cond) tuple, because the
// conditional pseudos carry their compare operands inline and have no
// reversible i8 condition vector. So analyzeBranch handles the unconditional
// and fall-through shapes and the "ends with a conditional pseudo we don't
// model" shapes it reports as unanalyzable (return true). That is enough for
// MachineBlockPlacement / BranchFolder, which only need to insert and remove
// UNCONDITIONAL branches when they rearrange blocks.
//===----------------------------------------------------------------------===//

bool I4004InstrInfo::analyzeBranch(MachineBasicBlock &MBB,
                                   MachineBasicBlock *&TBB,
                                   MachineBasicBlock *&FBB,
                                   SmallVectorImpl<MachineOperand> &Cond,
                                   bool AllowModify) const {
  TBB = FBB = nullptr;
  Cond.clear();

  MachineBasicBlock::iterator I = MBB.getLastNonDebugInstr();
  if (I == MBB.end() || !isUnpredicatedTerminator(*I))
    return false; // falls through; no terminator to analyze.

  // Find the first terminator in the run of trailing terminators.
  MachineBasicBlock::iterator FirstTerm = I;
  while (FirstTerm != MBB.begin()) {
    auto Prev = std::prev(FirstTerm);
    if (!isUnpredicatedTerminator(*Prev))
      break;
    FirstTerm = Prev;
  }

  // Classify each terminator. We model two conditional shapes exactly so the
  // generic BranchFolder / MachineBlockPlacement keep the CFG consistent (an
  // unmodelled conditional branch let BlockPlacement delete a block still
  // targeted by a JCN, producing dangling .LBB labels):
  //   BRCCi8/BRCCi16 <lhs>,<rhs>,<cc>,<tgt>   -> conditional
  //   BR <tgt>                                -> unconditional
  // Cond encoding: [opcode, ccimm, lhs, rhs] (4 operands).
  auto isCondBr = [](unsigned Op) {
    return Op == I4004::BRCCi8 || Op == I4004::BRCCi16 ||
           Op == I4004::BRCCi8ri || Op == I4004::BRCCi16ri ||
           Op == I4004::BRRAM;
  };

  for (auto T = FirstTerm; T != MBB.end(); ++T) {
    unsigned Op = T->getOpcode();
    if (T->isDebugInstr())
      continue;
    if (Op == I4004::BR) {
      // Unconditional branch. If a conditional was already recorded, this is
      // the false-path target (FBB); otherwise it is TBB.
      if (Cond.empty())
        TBB = T->getOperand(0).getMBB();
      else
        FBB = T->getOperand(0).getMBB();
      continue;
    }
    if (isCondBr(Op)) {
      // Only one conditional branch per block is modelled.
      if (!Cond.empty())
        return true;
      // Operands: lhs(0), rhs(1), cc-imm(2), target MBB(3).
      TBB = T->getOperand(3).getMBB();
      Cond.push_back(MachineOperand::CreateImm(Op));               // [0] opcode
      Cond.push_back(MachineOperand::CreateImm(T->getOperand(2).getImm())); // [1] cc
      Cond.push_back(T->getOperand(0));                            // [2] lhs
      Cond.push_back(T->getOperand(1));                            // [3] rhs
      continue;
    }
    // Any other terminator (BRZ/BRNZ, indirect, etc.): not modelled.
    return true;
  }

  // Consistency: a recorded conditional with no explicit BR falls through to
  // the layout successor (FBB stays null, which the generic code reads as
  // "fall through"). Pure-unconditional or fall-through are already set.
  return false;
}

bool I4004InstrInfo::reverseBranchCondition(
    SmallVectorImpl<MachineOperand> &Cond) const {
  if (Cond.size() != 4)
    return true; // cannot reverse
  // Cond[1] is the I4004CC code; flip to its complement.
  const int Used = Cond[1].getImm() & 0xF0;
  int CC = I4004CC::code(Cond[1].getImm());
  int Rev;
  switch (CC) {
  case I4004CC::EQ:  Rev = I4004CC::NE;  break;
  case I4004CC::NE:  Rev = I4004CC::EQ;  break;
  case I4004CC::ULT: Rev = I4004CC::UGE; break;
  case I4004CC::UGE: Rev = I4004CC::ULT; break;
  case I4004CC::UGT: Rev = I4004CC::ULE; break;
  case I4004CC::ULE: Rev = I4004CC::UGT; break;
  case I4004CC::SLT: Rev = I4004CC::SGE; break;
  case I4004CC::SGE: Rev = I4004CC::SLT; break;
  case I4004CC::NEG: Rev = I4004CC::NONNEG; break;
  case I4004CC::NONNEG: Rev = I4004CC::NEG; break;
  default: return true;
  }
  Cond[1].setImm(Rev | Used);
  return false;
}

unsigned I4004InstrInfo::insertBranch(MachineBasicBlock &MBB,
                                      MachineBasicBlock *TBB,
                                      MachineBasicBlock *FBB,
                                      ArrayRef<MachineOperand> Cond,
                                      const DebugLoc &DL,
                                      int *BytesAdded) const {
  assert(TBB && "insertBranch must have a true destination");
  int Bytes = 0;
  unsigned Count = 0;

  if (Cond.empty()) {
    // Unconditional branch to TBB.
    assert(!FBB && "unconditional branch cannot have a false target");
    BuildMI(&MBB, DL, get(I4004::BR)).addMBB(TBB);
    Bytes += 2;
    ++Count;
  } else {
    // Conditional: Cond = [opcode, cc, lhs, rhs]. Emit the BRCC to TBB.
    assert(Cond.size() == 4 && "malformed i4004 branch condition");
    unsigned Op = Cond[0].getImm();
    BuildMI(&MBB, DL, get(Op))
        .add(Cond[2])            // lhs
        .add(Cond[3])            // rhs
        .addImm(Cond[1].getImm()) // cc
        .addMBB(TBB);
    Bytes += 2;
    ++Count;
    // A false target needs a trailing unconditional branch.
    if (FBB) {
      BuildMI(&MBB, DL, get(I4004::BR)).addMBB(FBB);
      Bytes += 2;
      ++Count;
    }
  }
  if (BytesAdded)
    *BytesAdded = Bytes;
  return Count;
}

unsigned I4004InstrInfo::removeBranch(MachineBasicBlock &MBB,
                                      int *BytesRemoved) const {
  unsigned Count = 0;
  int Bytes = 0;
  MachineBasicBlock::iterator I = MBB.end();
  while (I != MBB.begin()) {
    --I;
    if (I->isDebugInstr())
      continue;
    // Only remove branches we model (BR and the conditional BRCC pseudos);
    // stop at the first non-(modeled)-terminator.
    unsigned Op = I->getOpcode();
    if (Op != I4004::BR && Op != I4004::BRCCi8 && Op != I4004::BRCCi16 &&
        Op != I4004::BRCCi8ri && Op != I4004::BRCCi16ri && Op != I4004::BRRAM)
      break;
    Bytes += 2;
    I->eraseFromParent();
    I = MBB.end();
    ++Count;
  }
  if (BytesRemoved)
    *BytesRemoved = Bytes;
  return Count;
}

unsigned I4004InstrInfo::getInstSizeInBytes(const MachineInstr &MI) const {
  if (MI.isMetaInstruction())
    return 0;
  return get(MI.getOpcode()).getSize();
}

//===----------------------------------------------------------------------===//
// MachineOutliner (issue #18). The outliner runs after I4004ExpandPseudos, on
// real MCS-4 instructions. Those do not model ACC as an operand, so its
// liveness is computed here from the opcodes: an outlined body returns with
// BBL 0, which loads ACC, so ACC must be dead after the sequence. CY and the
// index registers pass through JMS/BBL untouched.
//===----------------------------------------------------------------------===//

namespace {
/// How a call to an outlined body is built.
enum : unsigned { OutlineCall, OutlineTailCall };

enum class AccUse { None, Reads, Writes };

/// How an instruction uses ACC first: reads it (live before), or overwrites
/// it without reading (dead before).
AccUse accUse(const MachineInstr &MI) {
  switch (MI.getOpcode()) {
  case I4004::LD:
  case I4004::LDM:
  case I4004::CLB:
  case I4004::RDM:
  case I4004::RDR:
  case I4004::RD0:
  case I4004::RD1:
  case I4004::RD2:
  case I4004::RD3:
  case I4004::TCC:
  case I4004::TCS:
  case I4004::BBL:
    return AccUse::Writes;
  case I4004::JMS: {
    // No routine takes an argument in ACC (abi.md 4.1), but an outlined body
    // may continue a computation in ACC from its caller.
    const MachineOperand &T = MI.getOperand(0);
    if (T.isGlobal() && T.getGlobal()->getName().starts_with("OUTLINED_FUNCTION"))
      return AccUse::Reads;
    return AccUse::Writes;
  }
  case I4004::NOP:
  case I4004::FIM:
  case I4004::SRC:
  case I4004::INC:
  case I4004::CLC:
  case I4004::CMC:
  case I4004::STC:
    return AccUse::None;
  default: // ADD SUB XCH IAC DAC CMA RAL RAR DAA KBP ADM SBM, the writes
           // (WRM WR0-3 WRR WMP WPM DCL), JCN, and anything unknown
    return AccUse::Reads;
  }
}

/// Whether ACC may be read after `From` in `MBB` before being overwritten:
/// scan forward, then into the successors (Depth levels); unknown is live.
bool accLiveFrom(const MachineBasicBlock &MBB,
                 MachineBasicBlock::const_iterator From, unsigned Depth) {
  for (auto I = From, E = MBB.end(); I != E; ++I) {
    if (I->isMetaInstruction())
      continue;
    switch (accUse(*I)) {
    case AccUse::Reads:
      return true;
    case AccUse::Writes:
      return false;
    case AccUse::None:
      break;
    }
  }
  if (MBB.succ_empty())
    return false;
  if (Depth == 0)
    return true;
  for (const MachineBasicBlock *S : MBB.successors())
    if (accLiveFrom(*S, S->begin(), Depth - 1))
      return true;
  return false;
}
} // namespace

bool I4004InstrInfo::isFunctionSafeToOutlineFrom(
    MachineFunction &MF, bool OutlineFromLinkOnceODRs) const {
  if (i4004LegacyLayout())
    return false;
  const Function &F = MF.getFunction();
  if (!OutlineFromLinkOnceODRs && F.hasLinkOnceODRLinkage())
    return false;
  return !F.hasSection();
}

outliner::InstrType
I4004InstrInfo::getOutliningTypeImpl(const MachineModuleInfo &MMI,
                                     MachineBasicBlock::iterator &MIT,
                                     unsigned Flags) const {
  const MachineInstr &MI = *MIT;
  // A sequence may end in the function's return: it is then entered with JUN
  // (a tail call: no stack level, no extra BBL).
  if (MI.getOpcode() == I4004::BBL && MI.getOperand(0).getImm() == 0)
    return outliner::InstrType::LegalTerminator;
  switch (MI.getOpcode()) {
  // Control flow stays put: branches are page-local, calls would deepen the
  // hardware stack, FIN/JIN depend on the page they execute in.
  case I4004::JCN:
  case I4004::JUN:
  case I4004::JMS:
  case I4004::ISZ:
  case I4004::JIN:
  case I4004::FIN:
  case I4004::BBL:
  case I4004::DCL: // a bank bracket stays whole
    return outliner::InstrType::Illegal;
  default:
    break;
  }
  if (MI.isPseudo() || MI.isCall() || MI.isBranch() || MI.isReturn() ||
      MI.isTerminator() || MI.isInlineAsm())
    return outliner::InstrType::Illegal;
  return outliner::InstrType::Legal;
}

std::optional<std::unique_ptr<outliner::OutlinedFunction>>
I4004InstrInfo::getOutliningCandidateInfo(
    const MachineModuleInfo &MMI,
    std::vector<outliner::Candidate> &RepeatedSequenceLocs,
    unsigned MinRepeats) const {
  const bool TailCall =
      RepeatedSequenceLocs[0].back().getOpcode() == I4004::BBL;
  // A body entered by JMS returns with BBL 0, which sets ACC = 0: drop the
  // sites where ACC is live afterwards. (A tail call returns to the caller's
  // caller, where ACC is dead.)
  if (!TailCall)
    llvm::erase_if(RepeatedSequenceLocs, [](outliner::Candidate &C) {
      return accLiveFrom(*C.getMBB(), C.end(), /*Depth=*/2);
    });
  if (RepeatedSequenceLocs.size() < MinRepeats)
    return std::nullopt;
  unsigned SequenceSize = 0;
  for (const MachineInstr &MI : RepeatedSequenceLocs[0])
    SequenceSize += getInstSizeInBytes(MI);
  const unsigned ID = TailCall ? OutlineTailCall : OutlineCall;
  for (outliner::Candidate &C : RepeatedSequenceLocs)
    C.setCallInfo(ID, /*JMS or JUN=*/2);
  return std::make_unique<outliner::OutlinedFunction>(
      RepeatedSequenceLocs, SequenceSize, /*BBL=*/TailCall ? 0 : 1, ID);
}

void I4004InstrInfo::buildOutlinedFrame(
    MachineBasicBlock &MBB, MachineFunction &MF,
    const outliner::OutlinedFunction &OF) const {
  if (OF.FrameConstructionID == OutlineTailCall)
    return; // the body ends in the callers' own BBL 0
  MBB.insert(MBB.end(), BuildMI(MF, DebugLoc(), get(I4004::BBL)).addImm(0));
}

MachineBasicBlock::iterator I4004InstrInfo::insertOutlinedCall(
    Module &M, MachineBasicBlock &MBB, MachineBasicBlock::iterator &It,
    MachineFunction &MF, outliner::Candidate &C) const {
  const unsigned Opc =
      C.CallConstructionID == OutlineTailCall ? I4004::JUN : I4004::JMS;
  It = MBB.insert(It, BuildMI(MF, DebugLoc(), get(Opc))
                          .addGlobalAddress(M.getNamedValue(MF.getName())));
  return It;
}
