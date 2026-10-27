//===-- I8086AsmPrinter.cpp - I8086 LLVM Assembly Printer -------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086AsmPrinter.h"
#include "MCTargetDesc/I8086InstPrinter.h"
#include "I8086InstrInfo.h"
#include "I8086MCInstLower.h"
#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstBuilder.h"
#include "llvm/MC/MCStreamer.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-asm-printer"

char I8086AsmPrinter::ID = 0;

// `rep movsb` is a prefix instruction and a string instruction.
static bool repForm(unsigned Opc, unsigned &Op) {
  switch (Opc) {
  case I8086::P_REP_MOVSB: Op = I8086::MOVSB; return true;
  case I8086::P_REP_MOVSW: Op = I8086::MOVSW; return true;
  case I8086::P_REP_STOSB: Op = I8086::STOSB; return true;
  case I8086::P_REP_STOSW: Op = I8086::STOSW; return true;
  default: return false;
  }
}

bool I8086AsmPrinter::PrintAsmOperand(const MachineInstr *MI, unsigned OpNo,
                                      const char *ExtraCode, raw_ostream &OS) {
  if (ExtraCode && ExtraCode[0])
    return AsmPrinter::PrintAsmOperand(MI, OpNo, ExtraCode, OS);
  const MachineOperand &MO = MI->getOperand(OpNo);
  switch (MO.getType()) {
  case MachineOperand::MO_Register:
    OS << StringRef(I8086InstPrinter::getRegisterName(MO.getReg())).lower();
    return false;
  case MachineOperand::MO_Immediate:
    OS << MO.getImm();
    return false;
  default:
    return true;
  }
}

void I8086AsmPrinter::emitInstruction(const MachineInstr *MI) {
  unsigned StringOp;
  if (repForm(MI->getOpcode(), StringOp)) {
    EmitToStreamer(*OutStreamer, MCInstBuilder(I8086::REP));
    EmitToStreamer(*OutStreamer, MCInstBuilder(StringOp));
    return;
  }
  // A call to a far function in this segment pushes the segment half of the
  // return address first; `retf` pops it.
  if (MI->getOpcode() == I8086::P_CALLF) {
    EmitToStreamer(*OutStreamer,
                   MCInstBuilder(I8086::PUSH16s).addReg(I8086::CS));
  }
  I8086MCInstLower Lower(OutContext, *this);
  MCInst TmpInst;
  Lower.lower(MI, TmpInst);
  EmitToStreamer(*OutStreamer, TmpInst);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8086AsmPrinter() {
  RegisterAsmPrinter<I8086AsmPrinter> X(getTheI8086Target());
}
