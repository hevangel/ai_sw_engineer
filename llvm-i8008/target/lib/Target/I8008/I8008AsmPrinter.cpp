//===-- I8008AsmPrinter.cpp - I8008 LLVM Assembly Printer ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008AsmPrinter.h"
#include "I8008.h"
#include "I8008MCInstLower.h"
#include "I8008MachineFunctionInfo.h"
#include "TargetInfo/I8008TargetInfo.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCStreamer.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define DEBUG_TYPE "i8008-asm-printer"

char I8008AsmPrinter::ID = 0;

void I8008AsmPrinter::emitInstruction(const MachineInstr *MI) {
  I8008MCInstLower Lower(OutContext, *this);
  MCInst TmpInst;
  Lower.lower(MI, TmpInst);
  EmitToStreamer(*OutStreamer, TmpInst);
}

void I8008AsmPrinter::emitFunctionBodyEnd() {
  auto Publish = [&](const Twine &Name, uint64_t Bytes) {
    MCSymbol *Sym = OutContext.getOrCreateSymbol(Name);
    OutStreamer->emitSymbolAttribute(Sym, MCSA_Global);
    OutStreamer->emitAssignment(Sym, MCConstantExpr::create(Bytes, OutContext));
  };
  Publish("__framesize_" + MF->getName(), MF->getFrameInfo().getStackSize());
  Publish("__argsize_" + MF->getName(),
          MF->getInfo<I8008MachineFunctionInfo>()->ArgAreaSize);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8008AsmPrinter() {
  RegisterAsmPrinter<I8008AsmPrinter> X(getTheI8008Target());
}
