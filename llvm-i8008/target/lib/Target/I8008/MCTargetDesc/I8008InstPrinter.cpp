//===-- I8008InstPrinter.cpp - Convert I8008 MCInst to asm ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008InstPrinter.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/Support/Format.h"
#include "llvm/Support/raw_ostream.h"
using namespace llvm;

#define DEBUG_TYPE "asm-printer"

#include "I8008GenAsmWriter.inc"

void I8008InstPrinter::printInst(const MCInst *MI, uint64_t Address,
                                 StringRef Annot, const MCSubtargetInfo &STI,
                                 raw_ostream &O) {
  printInstruction(MI, Address, O);
  printAnnotation(O, Annot);
}

void I8008InstPrinter::printOperand(const MCInst *MI, unsigned OpNo,
                                    raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isReg())
    O << getRegisterName(Op.getReg());
  else if (Op.isImm())
    O << Op.getImm();
  else
    MAI.printExpr(O, *Op.getExpr());
}

void I8008InstPrinter::printImm8(const MCInst *MI, unsigned OpNo,
                                 raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isImm())
    O << format("0x%02x", (unsigned)(Op.getImm() & 0xff));
  else
    printOperand(MI, OpNo, O);
}

void I8008InstPrinter::printAddr16(const MCInst *MI, unsigned OpNo,
                                   raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isImm())
    O << format("0x%04x", (unsigned)(Op.getImm() & 0x3fff));
  else
    printOperand(MI, OpNo, O);
}

// Only pseudos carry a memabs operand; they never reach the printer after
// expansion, but -print-after dumps may show one.
void I8008InstPrinter::printMemAbs(const MCInst *MI, unsigned OpNo,
                                   raw_ostream &O) {
  printOperand(MI, OpNo, O);
  O << '+';
  printOperand(MI, OpNo + 1, O);
}
