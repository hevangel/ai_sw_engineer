//===-- I4004AsmPrinter.h - I4004 LLVM Assembly Printer --------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004ASMPRINTER_H
#define LLVM_LIB_TARGET_I4004_I4004ASMPRINTER_H

#include "llvm/CodeGen/AsmPrinter.h"

namespace llvm {

class I4004AsmPrinter : public AsmPrinter {
public:
  explicit I4004AsmPrinter(TargetMachine &TM,
                           std::unique_ptr<MCStreamer> Streamer)
      : AsmPrinter(TM, std::move(Streamer), ID) {}

  static char ID;

  StringRef getPassName() const override { return "I4004 Assembly Printer"; }

  void emitInstruction(const MachineInstr *MI) override;
};

} // end namespace llvm

#endif
