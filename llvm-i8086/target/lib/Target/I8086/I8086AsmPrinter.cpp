//===-- I8086AsmPrinter.cpp - I8086 LLVM Assembly Printer -------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086AsmPrinter.h"
#include "I8086MCInstLower.h"
#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCStreamer.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-asm-printer"

char I8086AsmPrinter::ID = 0;

void I8086AsmPrinter::emitInstruction(const MachineInstr *MI) {
  I8086MCInstLower Lower(OutContext, *this);
  MCInst TmpInst;
  Lower.lower(MI, TmpInst);
  EmitToStreamer(*OutStreamer, TmpInst);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8086AsmPrinter() {
  RegisterAsmPrinter<I8086AsmPrinter> X(getTheI8086Target());
}
