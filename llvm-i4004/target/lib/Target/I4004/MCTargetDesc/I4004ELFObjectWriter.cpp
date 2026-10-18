//===-- I4004ELFObjectWriter.cpp - I4004 ELF Writer ----------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I4004FixupKinds.h"
#include "MCTargetDesc/I4004MCTargetDesc.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/MC/MCELFObjectWriter.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCObjectWriter.h"
#include "llvm/MC/MCValue.h"
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;

// Private, unregistered machine number for the Intel 4004 (abi.md sec.5).
// If upstream ever assigns a real EM_*, this is the single place to change.
#ifndef EM_I4004
#define EM_I4004 0x4004
#endif

namespace {
class I4004ELFObjectWriter : public MCELFObjectTargetWriter {
public:
  I4004ELFObjectWriter(uint8_t OSABI)
      : MCELFObjectTargetWriter(/*Is64Bit=*/false, OSABI, EM_I4004,
                                /*HasRelocationAddend=*/true) {}
  ~I4004ELFObjectWriter() override = default;

protected:
  unsigned getRelocType(const MCFixup &Fixup, const MCValue &Target,
                        bool IsPCRel) const override {
    switch (Fixup.getKind()) {
    case FK_Data_1:
      return ELF::R_I4004_PAGE8;
    case FK_Data_2:
      return ELF::R_I4004_RAMADDR16;
    case I4004::fixup_addr12:
      return ELF::R_I4004_ADDR12;
    case I4004::fixup_page8:
      return ELF::R_I4004_PAGE8;
    case I4004::fixup_imm8:
      return ELF::R_I4004_IMM8;
    case I4004::fixup_fin8:
      return ELF::R_I4004_FIN8;
    default:
      llvm_unreachable("invalid fixup kind for I4004");
    }
  }
};
} // end anonymous namespace

std::unique_ptr<MCObjectTargetWriter>
llvm::createI4004ELFObjectWriter(uint8_t OSABI) {
  return std::make_unique<I4004ELFObjectWriter>(OSABI);
}
