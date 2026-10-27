//===-- I8086MCAsmInfo.cpp - I8086 asm properties -------------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086MCAsmInfo.h"
#include "llvm/TargetParser/Triple.h"
using namespace llvm;

void I8086MCAsmInfo::anchor() {}

I8086MCAsmInfo::I8086MCAsmInfo(const Triple &TT, const MCTargetOptions &Options)
    : MCAsmInfoELF(Options) {
  CodePointerSize = 2;
  CalleeSaveStackSlotSize = 2;
  IsLittleEndian = true;
  CommentString = ";";
  SupportsDebugInformation = false;
  AlignmentIsInBytes = true;
  UsesELFSectionDirectiveForBSS = true;
  UseAtForSpecifier = true;
  initializeAtSpecifiers({{I8086::S_SEG, "seg"}});
}
