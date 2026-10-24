//===-- I8008ELFObjectWriter.cpp - I8008 ELF Writer ----------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8008FixupKinds.h"
#include "MCTargetDesc/I8008MCTargetDesc.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/MC/MCELFObjectWriter.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCObjectWriter.h"
#include "llvm/MC/MCValue.h"
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;

namespace {
class I8008ELFObjectWriter : public MCELFObjectTargetWriter {
public:
  I8008ELFObjectWriter(uint8_t OSABI)
      : MCELFObjectTargetWriter(/*Is64Bit=*/false, OSABI, ELF::EM_I8008,
                                /*HasRelocationAddend=*/true) {}

protected:
  unsigned getRelocType(const MCFixup &Fixup, const MCValue &Target,
                        bool IsPCRel) const override {
    switch (Fixup.getKind()) {
    case FK_Data_1:
    case I8008::fixup_8:
      return ELF::R_I8008_8;
    case FK_Data_2:
    case I8008::fixup_16:
      return ELF::R_I8008_16;
    case I8008::fixup_lo8:
      return ELF::R_I8008_LO8;
    case I8008::fixup_hi8:
      return ELF::R_I8008_HI8;
    default:
      llvm_unreachable("invalid fixup kind for I8008");
    }
  }
};
} // end anonymous namespace

std::unique_ptr<MCObjectTargetWriter>
llvm::createI8008ELFObjectWriter(uint8_t OSABI) {
  return std::make_unique<I8008ELFObjectWriter>(OSABI);
}
