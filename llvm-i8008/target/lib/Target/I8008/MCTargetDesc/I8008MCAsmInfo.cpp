//===-- I8008MCAsmInfo.cpp - I8008 asm properties -------------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008MCAsmInfo.h"
#include "I8008MCTargetDesc.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/TargetParser/Triple.h"
using namespace llvm;

void I8008MCAsmInfo::anchor() {}

void I8008MCAsmInfo::printSpecifierExpr(raw_ostream &OS,
                                        const MCSpecifierExpr &Expr) const {
  switch (Expr.getSpecifier()) {
  case I8008::S_LO:
    OS << "%lo(";
    break;
  case I8008::S_HI:
    OS << "%hi(";
    break;
  default:
    OS << "%specifier" << Expr.getSpecifier() << '(';
    break;
  }
  printExpr(OS, *Expr.getSubExpr());
  OS << ')';
}

I8008MCAsmInfo::I8008MCAsmInfo(const Triple &TT, const MCTargetOptions &Options)
    : MCAsmInfoELF(Options) {
  CodePointerSize = 2;
  CalleeSaveStackSlotSize = 1;
  IsLittleEndian = true;
  CommentString = ";";
  SupportsDebugInformation = false;
  AlignmentIsInBytes = true;
  UsesELFSectionDirectiveForBSS = true;
}
