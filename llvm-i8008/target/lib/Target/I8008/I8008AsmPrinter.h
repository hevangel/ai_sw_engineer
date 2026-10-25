//===-- I8008AsmPrinter.h - I8008 LLVM Assembly Printer --------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008ASMPRINTER_H
#define LLVM_LIB_TARGET_I8008_I8008ASMPRINTER_H

#include "llvm/CodeGen/AsmPrinter.h"

namespace llvm {

class I8008AsmPrinter : public AsmPrinter {
public:
  explicit I8008AsmPrinter(TargetMachine &TM,
                           std::unique_ptr<MCStreamer> Streamer)
      : AsmPrinter(TM, std::move(Streamer), ID) {}

  static char ID;

  StringRef getPassName() const override { return "I8008 Assembly Printer"; }

  void emitInstruction(const MachineInstr *MI) override;

  // Publish __framesize_<fn> and __argsize_<fn> (bytes) as absolute global
  // symbols. The linker sizes each function's static frame and memory-argument
  // area from them (abi.md sec.3.1, sec.4), so an alloca reached only through
  // a pointer is still reserved at its full extent.
  void emitFunctionBodyEnd() override;
};

} // end namespace llvm

#endif
