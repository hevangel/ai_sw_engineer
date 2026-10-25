//===-- I8086MCInstLower.h - Lower MachineInstr to MCInst -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086MCINSTLOWER_H
#define LLVM_LIB_TARGET_I8086_I8086MCINSTLOWER_H

#include "llvm/Support/Compiler.h"

namespace llvm {

class AsmPrinter;
class MCContext;
class MCInst;
class MCOperand;
class MCSymbol;
class MachineInstr;
class MachineOperand;

/// Turns a selected "P_X" instruction into the machine instruction X
/// (I8086InstrCodeGen.td): tied operands collapse, an immediate picks the
/// shortest encoding, and a memory operand becomes base/index/disp/segment
/// with the base and index in the registers the hardware allows.
class I8086MCInstLower {
  MCContext &Ctx;
  AsmPrinter &Printer;

public:
  I8086MCInstLower(MCContext &ctx, AsmPrinter &printer)
      : Ctx(ctx), Printer(printer) {}

  void lower(const MachineInstr *MI, MCInst &OutMI) const;

private:
  MCOperand lowerOperand(const MachineOperand &MO) const;
  void lowerMem(const MachineInstr *MI, unsigned First, MCInst &OutMI) const;
};

} // end namespace llvm

#endif
