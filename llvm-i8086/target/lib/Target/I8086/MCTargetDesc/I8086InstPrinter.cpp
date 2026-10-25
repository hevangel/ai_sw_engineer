//===-- I8086InstPrinter.cpp - Convert I8086 MCInst to asm ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Intel syntax. Memory operands always carry their size ("byte ptr [bx+si+4]")
// except where the size is the register's (lea); numbers are decimal below ten
// and hexadecimal above.
//
//===----------------------------------------------------------------------===//

#include "I8086InstPrinter.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/Support/Format.h"
#include "llvm/Support/raw_ostream.h"
using namespace llvm;

#define DEBUG_TYPE "asm-printer"

#include "I8086GenAsmWriter.inc"

void I8086InstPrinter::printInst(const MCInst *MI, uint64_t Address,
                                 StringRef Annot, const MCSubtargetInfo &STI,
                                 raw_ostream &O) {
  printInstruction(MI, Address, O);
  printAnnotation(O, Annot);
}

static void printNumber(raw_ostream &O, int64_t V) {
  if (V < 0) {
    O << '-';
    V = -V;
  }
  if (V < 10)
    O << V;
  else
    O << format("0x%llx", (unsigned long long)V);
}

void I8086InstPrinter::printOperand(const MCInst *MI, unsigned OpNo,
                                    raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isReg())
    O << getRegisterName(Op.getReg());
  else if (Op.isImm())
    printNumber(O, Op.getImm());
  else
    MAI.printExpr(O, *Op.getExpr());
}

void I8086InstPrinter::printImm(const MCInst *MI, unsigned OpNo,
                                raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isImm()) {
    // Unsigned: an immediate is a bit pattern of its instruction's width.
    int64_t V = Op.getImm();
    if (V < 0)
      V &= 0xffff;
    printNumber(O, V);
  } else {
    printOperand(MI, OpNo, O);
  }
}

void I8086InstPrinter::printSImm8(const MCInst *MI, unsigned OpNo,
                                  raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isImm())
    printNumber(O, static_cast<int8_t>(Op.getImm()));
  else
    printOperand(MI, OpNo, O);
}

void I8086InstPrinter::printBrTarget(const MCInst *MI, unsigned OpNo,
                                     raw_ostream &O) {
  const MCOperand &Op = MI->getOperand(OpNo);
  if (Op.isImm())
    O << format("0x%x", (unsigned)(Op.getImm() & 0xffff));
  else
    printOperand(MI, OpNo, O);
}

void I8086InstPrinter::printFarPtr(const MCInst *MI, unsigned OpNo,
                                   raw_ostream &O) {
  // Operands are (offset, segment); the syntax is segment:offset.
  O << format("0x%x:0x%x", (unsigned)(MI->getOperand(OpNo + 1).getImm() & 0xffff),
              (unsigned)(MI->getOperand(OpNo).getImm() & 0xffff));
}

void I8086InstPrinter::printMem(const MCInst *MI, unsigned OpNo, raw_ostream &O,
                                const char *Size) {
  const MCOperand &Base = MI->getOperand(OpNo);
  const MCOperand &Index = MI->getOperand(OpNo + 1);
  const MCOperand &Disp = MI->getOperand(OpNo + 2);
  const MCOperand &Seg = MI->getOperand(OpNo + 3);
  if (*Size)
    O << Size << " ptr ";
  if (Seg.getReg())
    O << getRegisterName(Seg.getReg()) << ':';
  O << '[';
  bool Any = false;
  if (Base.getReg()) {
    O << getRegisterName(Base.getReg());
    Any = true;
  }
  if (Index.getReg()) {
    if (Any)
      O << '+';
    O << getRegisterName(Index.getReg());
    Any = true;
  }
  if (Disp.isImm()) {
    int64_t V = static_cast<int16_t>(Disp.getImm() & 0xffff);
    if (!Any)
      O << format("0x%x", (unsigned)(Disp.getImm() & 0xffff));
    else if (V != 0) {
      O << (V < 0 ? '-' : '+');
      printNumber(O, V < 0 ? -V : V);
    }
  } else {
    if (Any)
      O << '+';
    MAI.printExpr(O, *Disp.getExpr());
  }
  O << ']';
}

void I8086InstPrinter::printMem8(const MCInst *MI, unsigned OpNo,
                                 raw_ostream &O) {
  printMem(MI, OpNo, O, "byte");
}
void I8086InstPrinter::printMem16(const MCInst *MI, unsigned OpNo,
                                  raw_ostream &O) {
  printMem(MI, OpNo, O, "word");
}
void I8086InstPrinter::printMem32(const MCInst *MI, unsigned OpNo,
                                  raw_ostream &O) {
  printMem(MI, OpNo, O, "dword");
}
void I8086InstPrinter::printMemNoSize(const MCInst *MI, unsigned OpNo,
                                      raw_ostream &O) {
  printMem(MI, OpNo, O, "");
}
