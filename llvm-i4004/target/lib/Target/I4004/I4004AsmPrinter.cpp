//===-- I4004AsmPrinter.cpp - I4004 LLVM Assembly Printer ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004AsmPrinter.h"
#include "I4004.h"
#include "I4004MCInstLower.h"
#include "MCTargetDesc/I4004InstPrinter.h"
#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCStreamer.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-asm-printer"

char I4004AsmPrinter::ID = 0;

void I4004AsmPrinter::emitInstruction(const MachineInstr *MI) {
  I4004MCInstLower Lower(OutContext, *this);
  MCInst TmpInst;
  Lower.lower(MI, TmpInst);
  EmitToStreamer(*OutStreamer, TmpInst);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI4004AsmPrinter() {
  RegisterAsmPrinter<I4004AsmPrinter> X(getTheI4004Target());
}
