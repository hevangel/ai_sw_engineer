//===-- I4004MCAsmInfo.cpp - I4004 asm properties -------------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004MCAsmInfo.h"
#include "I4004MCTargetDesc.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/TargetParser/Triple.h"
using namespace llvm;

void I4004MCAsmInfo::anchor() {}

// Print a %lo8(...) / %page(...) specifier expression so an assembled file can
// be disassembled/re-printed round-trip (the MC round-trip lit test).
void I4004MCAsmInfo::printSpecifierExpr(raw_ostream &OS,
                                        const MCSpecifierExpr &Expr) const {
  switch (Expr.getSpecifier()) {
  case I4004::S_LO8:
    OS << "%lo8(";
    break;
  case I4004::S_PAGE:
    OS << "%page(";
    break;
  default:
    OS << "%specifier" << Expr.getSpecifier() << '(';
    break;
  }
  printExpr(OS, *Expr.getSubExpr());
  OS << ')';
}

I4004MCAsmInfo::I4004MCAsmInfo(const Triple &TT, const MCTargetOptions &Options)
    : MCAsmInfoELF(Options) {
  CodePointerSize = 2;         // 16-bit software handle (abi.md sec.1.3)
  CalleeSaveStackSlotSize = 1;
  IsLittleEndian = true;

  // MCS-4 manual comment style is ';' (asm_syntax.md). '#' and '//' are also
  // accepted by the lexer via the default ELF settings.
  CommentString = ";";
  SupportsDebugInformation = false;

  // The 4004 is byte addressed; alignment is expressed in bytes.
  AlignmentIsInBytes = true;
  UsesELFSectionDirectiveForBSS = true;
}
