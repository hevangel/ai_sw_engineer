//===-- I8008AsmBackend.cpp - I8008 Assembler Backend --------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8008FixupKinds.h"
#include "MCTargetDesc/I8008MCTargetDesc.h"
#include "llvm/ADT/StringSwitch.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/MC/MCAsmBackend.h"
#include "llvm/MC/MCAssembler.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCELFObjectWriter.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCObjectWriter.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/MCValue.h"
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;

namespace {
class I8008AsmBackend : public MCAsmBackend {
  const MCSubtargetInfo &STI;
  uint8_t OSABI;

public:
  I8008AsmBackend(const MCSubtargetInfo &STI, uint8_t OSABI)
      : MCAsmBackend(llvm::endianness::little), STI(STI), OSABI(OSABI) {}

  void applyFixup(const MCFragment &, const MCFixup &, const MCValue &Target,
                  uint8_t *Data, uint64_t Value, bool IsResolved) override;

  std::unique_ptr<MCObjectTargetWriter>
  createObjectTargetWriter() const override {
    return createI8008ELFObjectWriter(OSABI);
  }

  std::optional<MCFixupKind> getFixupKind(StringRef Name) const override {
    unsigned Type = StringSwitch<unsigned>(Name)
#define ELF_RELOC(NAME, ID) .Case(#NAME, ID)
#include "llvm/BinaryFormat/ELFRelocs/I8008.def"
#undef ELF_RELOC
                        .Case("BFD_RELOC_NONE", ELF::R_I8008_NONE)
                        .Default(-1u);
    if (Type != -1u)
      return static_cast<MCFixupKind>(FirstLiteralRelocationKind + Type);
    return std::nullopt;
  }

  MCFixupKindInfo getFixupKindInfo(MCFixupKind Kind) const override {
    const static MCFixupKindInfo Infos[I8008::NumTargetFixupKinds] = {
        {"fixup_16", 0, 16, 0},
        {"fixup_lo8", 0, 8, 0},
        {"fixup_hi8", 0, 8, 0},
        {"fixup_8", 0, 8, 0},
    };
    if (mc::isRelocation(Kind))
      return {};
    if (Kind < FirstTargetFixupKind)
      return MCAsmBackend::getFixupKindInfo(Kind);
    return Infos[Kind - FirstTargetFixupKind];
  }

  bool writeNopData(raw_ostream &OS, uint64_t Count,
                    const MCSubtargetInfo *STI) const override {
    // The 8008 has no NOP; `lAA` (0xC0) leaves every register and flag alone.
    for (uint64_t I = 0; I < Count; ++I)
      OS << char(0xC0);
    return true;
  }
};

void I8008AsmBackend::applyFixup(const MCFragment &F, const MCFixup &Fixup,
                                 const MCValue &Target, uint8_t *Data,
                                 uint64_t Value, bool IsResolved) {
  maybeAddReloc(F, Fixup, Target, Value, IsResolved);
  if (!IsResolved)
    return;
  switch (unsigned Kind = Fixup.getKind()) {
  case FK_Data_1:
  case I8008::fixup_8:
  case I8008::fixup_lo8:
    Data[0] = Value & 0xff;
    break;
  case I8008::fixup_hi8:
    Data[0] = (Value >> 8) & 0xff;
    break;
  case FK_Data_2:
  case I8008::fixup_16:
    Data[0] = Value & 0xff;
    Data[1] = (Value >> 8) & 0xff;
    break;
  default:
    if (Kind >= FirstTargetFixupKind)
      llvm_unreachable("unknown I8008 fixup kind");
    break;
  }
}

} // end anonymous namespace

MCAsmBackend *llvm::createI8008MCAsmBackend(const Target &T,
                                            const MCSubtargetInfo &STI,
                                            const MCRegisterInfo &MRI,
                                            const MCTargetOptions &Options) {
  return new I8008AsmBackend(STI, ELF::ELFOSABI_STANDALONE);
}
