//===-- I8008MCInstLower.cpp - Lower MachineInstr to MCInst --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008MCInstLower.h"
#include "I8008InstrInfo.h"
#include "MCTargetDesc/I8008Specifiers.h"
#include "llvm/CodeGen/AsmPrinter.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/CodeGen/MachineOperand.h"
#include "llvm/IR/Constants.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"

using namespace llvm;

// Symbol + offset, wrapped in %lo / %hi per the operand's target flag.
MCOperand I8008MCInstLower::lowerSymbolOperand(const MachineOperand &MO,
                                               MCSymbol *Sym) const {
  const MCExpr *Expr = MCSymbolRefExpr::create(Sym, Ctx);
  if (!MO.isMBB() && !MO.isMCSymbol() && MO.getOffset())
    Expr = MCBinaryExpr::createAdd(
        Expr, MCConstantExpr::create(MO.getOffset(), Ctx), Ctx);
  if (MO.getTargetFlags() == I8008II::MO_LO)
    Expr = MCSpecifierExpr::create(Expr, I8008::S_LO, Ctx);
  else if (MO.getTargetFlags() == I8008II::MO_HI)
    Expr = MCSpecifierExpr::create(Expr, I8008::S_HI, Ctx);
  return MCOperand::createExpr(Expr);
}

MCOperand I8008MCInstLower::lowerOperand(const MachineOperand &MO) const {
  switch (MO.getType()) {
  case MachineOperand::MO_Register:
    break; // all register operands are implicit in the mnemonic
  case MachineOperand::MO_Immediate:
    return MCOperand::createImm(MO.getImm());
  case MachineOperand::MO_MachineBasicBlock:
    return lowerSymbolOperand(MO, MO.getMBB()->getSymbol());
  case MachineOperand::MO_MCSymbol:
    return lowerSymbolOperand(MO, MO.getMCSymbol());
  case MachineOperand::MO_GlobalAddress:
    return lowerSymbolOperand(MO, Printer.getSymbol(MO.getGlobal()));
  case MachineOperand::MO_ExternalSymbol:
    return lowerSymbolOperand(MO,
                              Printer.GetExternalSymbolSymbol(MO.getSymbolName()));
  case MachineOperand::MO_BlockAddress:
    return lowerSymbolOperand(MO,
                              Printer.GetBlockAddressSymbol(MO.getBlockAddress()));
  case MachineOperand::MO_RegisterMask:
    break;
  default:
    break;
  }
  return MCOperand();
}

void I8008MCInstLower::lower(const MachineInstr *MI, MCInst &OutMI) const {
  OutMI.setOpcode(MI->getOpcode());
  for (const MachineOperand &MO : MI->operands()) {
    MCOperand MCOp = lowerOperand(MO);
    if (MCOp.isValid())
      OutMI.addOperand(MCOp);
  }
}
