//===-- I8086MCInstLower.cpp - Lower MachineInstr to MCInst ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086MCInstLower.h"
#include "I8086InstrInfo.h"
#include "llvm/ADT/StringMap.h"
#include "llvm/CodeGen/AsmPrinter.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/CodeGen/MachineOperand.h"
#include "llvm/IR/Constants.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"

using namespace llvm;

namespace {

/// Opcode of the machine instruction a P_X stands for, found by name.
unsigned realOpcode(const MCInstrInfo &MII, unsigned Pseudo) {
  static StringMap<unsigned> ByName;
  if (ByName.empty())
    for (unsigned Op = 0, E = MII.getNumOpcodes(); Op != E; ++Op)
      ByName[MII.getName(Op)] = Op;
  StringRef Name = MII.getName(Pseudo);
  assert(Name.starts_with("P_") && "not a code generation instruction");
  auto It = ByName.find(Name.drop_front(2));
  assert(It != ByName.end() && "no machine instruction for this pseudo");
  return It->second;
}

// The Jcc opcode for each hardware condition code (the relaxable form).
unsigned jccOpcode(int CC) {
  static const unsigned Tab[16] = {
      I8086::JO_R,  I8086::JNO_R, I8086::JB_R,  I8086::JAE_R,
      I8086::JE_R,  I8086::JNE_R, I8086::JBE_R, I8086::JA_R,
      I8086::JS_R,  I8086::JNS_R, I8086::JP_R,  I8086::JNP_R,
      I8086::JL_R,  I8086::JGE_R, I8086::JLE_R, I8086::JG_R};
  return Tab[CC & 15];
}

// ALU operations with an immediate: the pseudo and the machine forms from
// shortest to general.
struct AluImm {
  unsigned P8, P16;
  unsigned R8ri, R8ai, R16ri, R16ri8, R16ai;
};
#define ALU_IMM(N)                                                             \
  {I8086::P_##N##8ri, I8086::P_##N##16ri, I8086::N##8ri, I8086::N##8ai,       \
   I8086::N##16ri,    I8086::N##16ri8,    I8086::N##16ai}
const AluImm AluImms[] = {ALU_IMM(ADD), ALU_IMM(OR),  ALU_IMM(AND), ALU_IMM(ADC), ALU_IMM(SBB),
                          ALU_IMM(SUB), ALU_IMM(XOR), ALU_IMM(CMP)};
#undef ALU_IMM

} // namespace

// An 8-bit immediate is the byte, whatever the sign of the value the DAG held.
static MCOperand byteImm(MCOperand MO) {
  if (MO.isImm())
    return MCOperand::createImm(MO.getImm() & 0xFF);
  return MO;
}

MCOperand I8086MCInstLower::lowerOperand(const MachineOperand &MO) const {
  auto Sym = [&](MCSymbol *S) {
    const MCExpr *E = MCSymbolRefExpr::create(S, Ctx);
    if (!MO.isMBB() && !MO.isMCSymbol() && MO.getOffset())
      E = MCBinaryExpr::createAdd(E, MCConstantExpr::create(MO.getOffset(), Ctx),
                                  Ctx);
    return MCOperand::createExpr(E);
  };
  switch (MO.getType()) {
  case MachineOperand::MO_Register:
    return MCOperand::createReg(MO.getReg());
  case MachineOperand::MO_Immediate:
    return MCOperand::createImm(MO.getImm());
  case MachineOperand::MO_MachineBasicBlock:
    return Sym(MO.getMBB()->getSymbol());
  case MachineOperand::MO_MCSymbol:
    return Sym(MO.getMCSymbol());
  case MachineOperand::MO_GlobalAddress:
    return Sym(Printer.getSymbol(MO.getGlobal()));
  case MachineOperand::MO_ExternalSymbol:
    return Sym(Printer.GetExternalSymbolSymbol(MO.getSymbolName()));
  case MachineOperand::MO_BlockAddress:
    return Sym(Printer.GetBlockAddressSymbol(MO.getBlockAddress()));
  default:
    llvm_unreachable("unexpected operand kind");
  }
}

// The four operands of a memory reference at First: base, index, disp, seg.
// The hardware has base BX/BP and index SI/DI; selection leaves a lone
// pointer in the base slot whatever its register.
void I8086MCInstLower::lowerMem(const MachineInstr *MI, unsigned First,
                                MCInst &OutMI) const {
  MCRegister Base = MI->getOperand(First).getReg();
  MCRegister Index = MI->getOperand(First + 1).getReg();
  if (!Index && (Base == I8086::SI || Base == I8086::DI))
    std::swap(Base, Index);
  assert((!Base || Base == I8086::BX || Base == I8086::BP) &&
         "base register must be BX or BP");
  assert((!Index || Index == I8086::SI || Index == I8086::DI) &&
         "index register must be SI or DI");
  OutMI.addOperand(MCOperand::createReg(Base));
  OutMI.addOperand(MCOperand::createReg(Index));
  OutMI.addOperand(lowerOperand(MI->getOperand(First + 2)));
  OutMI.addOperand(MCOperand::createReg(MI->getOperand(First + 3).getReg()));
}

void I8086MCInstLower::lower(const MachineInstr *MI, MCInst &OutMI) const {
  const MCInstrDesc &Desc = MI->getDesc();
  const TargetInstrInfo &TII = *MI->getMF()->getSubtarget().getInstrInfo();
  unsigned Opc = MI->getOpcode();
  StringRef Name = TII.getName(Opc);

  // The declared operands, minus a use tied to a definition: the machine
  // instruction names that register once.
  SmallVector<unsigned, 8> Ops;
  for (unsigned I = 0, E = Desc.getNumOperands(); I != E; ++I)
    if (Desc.getOperandConstraint(I, MCOI::TIED_TO) < 0)
      Ops.push_back(I);

  switch (Opc) {
  case I8086::P_JMP:
    OutMI.setOpcode(I8086::JMP_R);
    OutMI.addOperand(lowerOperand(MI->getOperand(0)));
    return;
  case I8086::P_JCC:
    OutMI.setOpcode(jccOpcode(MI->getOperand(1).getImm()));
    OutMI.addOperand(lowerOperand(MI->getOperand(0)));
    return;
  case I8086::P_CALL:
    OutMI.setOpcode(I8086::CALL_N);
    OutMI.addOperand(lowerOperand(MI->getOperand(0)));
    return;
  case I8086::P_CALLr:
    OutMI.setOpcode(I8086::CALL16r);
    OutMI.addOperand(lowerOperand(MI->getOperand(0)));
    return;
  case I8086::P_RET:
    OutMI.setOpcode(I8086::RET);
    return;
  // Machine instructions the frame lowering emits directly (interrupt handlers).
  case I8086::PUSH16s:
  case I8086::POP16s:
    OutMI.setOpcode(Opc);
    OutMI.addOperand(lowerOperand(MI->getOperand(0)));
    return;
  case I8086::CLD:
  case I8086::IRET:
    OutMI.setOpcode(Opc);
    return;
  default:
    break;
  }

  // ALU with an immediate: the sign-extended byte form, the accumulator form
  // or the general one, whichever is shortest.
  for (const AluImm &A : AluImms) {
    if (Opc != A.P8 && Opc != A.P16)
      continue;
    MCRegister Reg = MI->getOperand(0).getReg();
    const MachineOperand &Imm = MI->getOperand(Ops.back());
    unsigned Op;
    if (Opc == A.P8)
      Op = Reg == I8086::AL ? A.R8ai : A.R8ri;
    else if (Imm.isImm() && isInt<8>(Imm.getImm()))
      Op = A.R16ri8;
    else
      Op = Reg == I8086::AX ? A.R16ai : A.R16ri;
    OutMI.setOpcode(Op);
    OutMI.addOperand(MCOperand::createReg(Reg));
    OutMI.addOperand(Opc == A.P8 ? byteImm(lowerOperand(Imm)) : lowerOperand(Imm));
    return;
  }

  OutMI.setOpcode(realOpcode(TII, Opc));
  // The accumulator has short forms for an absolute address (no base or
  // index), the same choice the assembler makes.
  {
    auto absolute = [&](unsigned First) {
      return !MI->getOperand(First).getReg() && !MI->getOperand(First + 1).getReg();
    };
    switch (Opc) {
    case I8086::P_MOV8rm:
    case I8086::P_MOV16rm:
      if (MI->getOperand(0).getReg() == (Opc == I8086::P_MOV8rm ? I8086::AL : I8086::AX) &&
          absolute(1))
        OutMI.setOpcode(Opc == I8086::P_MOV8rm ? I8086::MOV8ao : I8086::MOV16ao);
      break;
    case I8086::P_MOV8mr:
    case I8086::P_MOV16mr:
      if (MI->getOperand(4).getReg() == (Opc == I8086::P_MOV8mr ? I8086::AL : I8086::AX) &&
          absolute(0))
        OutMI.setOpcode(Opc == I8086::P_MOV8mr ? I8086::MOV8oa : I8086::MOV16oa);
      break;
    default:
      break;
    }
  }
  // The memory reference, if any, starts at the first GR16_PTR operand.
  int Mem = -1;
  for (unsigned I : Ops)
    if (Desc.operands()[I].RegClass == I8086::GR16_PTRRegClassID) {
      Mem = int(I);
      break;
    }
  for (unsigned I : Ops) {
    if (Mem >= 0 && I == unsigned(Mem)) {
      lowerMem(MI, I, OutMI);
      continue;
    }
    if (Mem >= 0 && I > unsigned(Mem) && I < unsigned(Mem) + 4)
      continue; // part of the memory reference
    MCOperand MO = lowerOperand(MI->getOperand(I));
    if (Name.ends_with("8ri") || Name.ends_with("8mi"))
      MO = byteImm(MO);
    OutMI.addOperand(MO);
  }
  // Shifts name their count: one, or CL.
  if (Name.ends_with("r1"))
    OutMI.addOperand(MCOperand::createImm(1));
  else if (Name.ends_with("rCL"))
    OutMI.addOperand(MCOperand::createReg(I8086::CL));
}
