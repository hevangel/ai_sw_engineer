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
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCStreamer.h"
#include "llvm/MC/MCSymbol.h"
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

void I4004AsmPrinter::emitFunctionBodyEnd() {
  // Publish this function's total frame size (in 4002 chars = stack bytes * 2)
  // as an absolute, GLOBAL symbol __framesize_<fn>. The packer reads it so the
  // per-function RAM frame is reserved at its full extent -- a large alloca
  // reached only through a base pointer (LEA_FI + memset/memcpy) is otherwise
  // invisible to the reloc-offset scan and the next frame would overlap it.
  const MachineFrameInfo &MFI = MF->getFrameInfo();
  uint64_t Chars = (uint64_t)MFI.getStackSize() * 2;
  MCSymbol *Sym = OutContext.getOrCreateSymbol(
      Twine("__framesize_") + MF->getName());
  OutStreamer->emitSymbolAttribute(Sym, MCSA_Global);
  OutStreamer->emitAssignment(
      Sym, MCConstantExpr::create((int64_t)Chars, OutContext));
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI4004AsmPrinter() {
  RegisterAsmPrinter<I4004AsmPrinter> X(getTheI4004Target());
}
