//===-- I8086InstPrinter.h - Convert I8086 MCInst to asm -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086INSTPRINTER_H
#define LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086INSTPRINTER_H

#include "llvm/MC/MCInstPrinter.h"

namespace llvm {

class I8086InstPrinter : public MCInstPrinter {
public:
  I8086InstPrinter(const MCAsmInfo &MAI, const MCInstrInfo &MII,
                   const MCRegisterInfo &MRI)
      : MCInstPrinter(MAI, MII, MRI) {}

  void printInst(const MCInst *MI, uint64_t Address, StringRef Annot,
                 const MCSubtargetInfo &STI, raw_ostream &O) override;
  std::pair<const char *, uint64_t> getMnemonic(const MCInst &MI) const override;

  void printInstruction(const MCInst *MI, uint64_t Address, raw_ostream &O);
  static const char *getRegisterName(MCRegister Reg);

  void printOperand(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printImm(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printSImm8(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printBrTarget(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printFarPtr(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printMem8(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printMem16(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printMem32(const MCInst *MI, unsigned OpNo, raw_ostream &O);
  void printMemNoSize(const MCInst *MI, unsigned OpNo, raw_ostream &O);

private:
  void printMem(const MCInst *MI, unsigned OpNo, raw_ostream &O,
                const char *Size);
};

} // namespace llvm

#endif
