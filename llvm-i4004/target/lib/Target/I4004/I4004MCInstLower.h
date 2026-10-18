//===-- I4004MCInstLower.h - Lower MachineInstr to MCInst ------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004MCINSTLOWER_H
#define LLVM_LIB_TARGET_I4004_I4004MCINSTLOWER_H

#include "llvm/Support/Compiler.h"

namespace llvm {

class AsmPrinter;
class MCContext;
class MCInst;
class MCOperand;
class MCSymbol;
class MachineInstr;
class MachineOperand;

class I4004MCInstLower {
  MCContext &Ctx;
  AsmPrinter &Printer;

public:
  I4004MCInstLower(MCContext &ctx, AsmPrinter &printer)
      : Ctx(ctx), Printer(printer) {}

  void lower(const MachineInstr *MI, MCInst &OutMI) const;
  MCOperand lowerOperand(const MachineOperand &MO) const;
  MCOperand lowerSymbolOperand(const MachineOperand &MO, MCSymbol *Sym) const;
};

} // end namespace llvm

#endif
