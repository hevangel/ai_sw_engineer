//===-- I8086AsmPrinter.h - I8086 LLVM Assembly Printer ----------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086ASMPRINTER_H
#define LLVM_LIB_TARGET_I8086_I8086ASMPRINTER_H

#include "llvm/CodeGen/AsmPrinter.h"

namespace llvm {

class I8086AsmPrinter : public AsmPrinter {
public:
  explicit I8086AsmPrinter(TargetMachine &TM,
                           std::unique_ptr<MCStreamer> Streamer)
      : AsmPrinter(TM, std::move(Streamer), ID) {}

  static char ID;

  StringRef getPassName() const override { return "I8086 Assembly Printer"; }

  void emitInstruction(const MachineInstr *MI) override;

  /// Operands of inline assembly: a register by name, an immediate by value.
  bool PrintAsmOperand(const MachineInstr *MI, unsigned OpNo,
                       const char *ExtraCode, raw_ostream &OS) override;
};

} // end namespace llvm

#endif
