//===-- I8086ELFObjectWriter.cpp - I8086 ELF Writer -----------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8086FixupKinds.h"
#include "MCTargetDesc/I8086MCAsmInfo.h"
#include "MCTargetDesc/I8086MCTargetDesc.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/MC/MCELFObjectWriter.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCObjectWriter.h"
#include "llvm/MC/MCValue.h"
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;

namespace {
class I8086ELFObjectWriter : public MCELFObjectTargetWriter {
public:
  I8086ELFObjectWriter(uint8_t OSABI)
      : MCELFObjectTargetWriter(/*Is64Bit=*/false, OSABI, ELF::EM_I8086,
                                /*HasRelocationAddend=*/true) {}

protected:
  unsigned getRelocType(const MCFixup &Fixup, const MCValue &Target,
                        bool IsPCRel) const override {
    switch (Fixup.getKind()) {
    case FK_Data_1:
    case I8086::fixup_8:
      return ELF::R_I8086_8;
    case FK_Data_2:
      // `.word sym@seg`
      if (Target.getSpecifier() == I8086::S_SEG)
        return ELF::R_I8086_SEG16;
      return ELF::R_I8086_16;
    case I8086::fixup_16:
      return ELF::R_I8086_16;
    case I8086::fixup_pc8:
      return ELF::R_I8086_PC8;
    case I8086::fixup_pc16:
      return ELF::R_I8086_PC16;
    case I8086::fixup_seg16:
      return ELF::R_I8086_SEG16;
    default:
      llvm_unreachable("invalid fixup kind for I8086");
    }
  }
};
} // end anonymous namespace

std::unique_ptr<MCObjectTargetWriter>
llvm::createI8086ELFObjectWriter(uint8_t OSABI) {
  return std::make_unique<I8086ELFObjectWriter>(OSABI);
}
