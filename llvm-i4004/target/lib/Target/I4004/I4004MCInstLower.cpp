//===-- I4004MCInstLower.cpp - Lower MachineInstr to MCInst --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004MCInstLower.h"
#include "I4004InstrInfo.h"
#include "MCTargetDesc/I4004Specifiers.h"
#include "llvm/CodeGen/AsmPrinter.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/CodeGen/MachineOperand.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"

using namespace llvm;

MCOperand I4004MCInstLower::lowerSymbolOperand(const MachineOperand &MO,
                                               MCSymbol *Sym) const {
  const MCExpr *Expr = MCSymbolRefExpr::create(Sym, Ctx);
  if (MO.getOffset())
    Expr = MCBinaryExpr::createAdd(
        Expr, MCConstantExpr::create(MO.getOffset(), Ctx), Ctx);
  return MCOperand::createExpr(Expr);
}

MCOperand I4004MCInstLower::lowerOperand(const MachineOperand &MO) const {
  switch (MO.getType()) {
  case MachineOperand::MO_Register:
    if (MO.isImplicit())
      break;
    return MCOperand::createReg(MO.getReg());
  case MachineOperand::MO_Immediate:
    return MCOperand::createImm(MO.getImm());
  case MachineOperand::MO_MachineBasicBlock: {
    const MCExpr *Expr =
        MCSymbolRefExpr::create(MO.getMBB()->getSymbol(), Ctx);
    // A soft-call FIM references the return block with a %lo8/%page specifier.
    unsigned TF = MO.getTargetFlags();
    if (TF == I4004II::MO_LO8)
      Expr = MCSpecifierExpr::create(Expr, I4004::S_LO8, Ctx);
    else if (TF == I4004II::MO_PAGE)
      Expr = MCSpecifierExpr::create(Expr, I4004::S_PAGE, Ctx);
    return MCOperand::createExpr(Expr);
  }
  case MachineOperand::MO_GlobalAddress:
    return lowerSymbolOperand(MO, Printer.getSymbol(MO.getGlobal()));
  case MachineOperand::MO_ExternalSymbol:
    return lowerSymbolOperand(
        MO, Printer.GetExternalSymbolSymbol(MO.getSymbolName()));
  default:
    break;
  }
  return MCOperand();
}

void I4004MCInstLower::lower(const MachineInstr *MI, MCInst &OutMI) const {
  OutMI.setOpcode(MI->getOpcode());
  for (const MachineOperand &MO : MI->operands()) {
    MCOperand MCOp = lowerOperand(MO);
    if (MCOp.isValid())
      OutMI.addOperand(MCOp);
  }
}
