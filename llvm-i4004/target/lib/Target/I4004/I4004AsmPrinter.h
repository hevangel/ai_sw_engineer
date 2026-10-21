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

  // Emit a per-function absolute symbol __framesize_<fn> = total stack size in
  // 4002 characters (bytes*2). The packer's frame-overlay allocator reads it to
  // size each __frame_<fn>, so a large alloca accessed only by a base pointer
  // (LEA_FI + memset/memcpy) is counted at its FULL extent, not just its base
  // reloc -- otherwise the next frame overlaps it and the firmware corrupts RAM.
  void emitFunctionBodyEnd() override;
};

} // end namespace llvm

#endif
