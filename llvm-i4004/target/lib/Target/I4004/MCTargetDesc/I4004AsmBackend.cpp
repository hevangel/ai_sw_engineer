//===-- I4004AsmBackend.cpp - I4004 Assembler Backend --------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I4004FixupKinds.h"
#include "MCTargetDesc/I4004MCTargetDesc.h"
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
#include "llvm/Support/raw_ostream.h"

using namespace llvm;

namespace {
class I4004AsmBackend : public MCAsmBackend {
  uint8_t OSABI;

public:
  I4004AsmBackend(const MCSubtargetInfo &STI, uint8_t OSABI)
      : MCAsmBackend(llvm::endianness::little), OSABI(OSABI) {}
  ~I4004AsmBackend() override = default;

  void applyFixup(const MCFragment &, const MCFixup &, const MCValue &Target,
                  uint8_t *Data, uint64_t Value, bool IsResolved) override;

  std::unique_ptr<MCObjectTargetWriter>
  createObjectTargetWriter() const override {
    return createI4004ELFObjectWriter(OSABI);
  }

  MCFixupKindInfo getFixupKindInfo(MCFixupKind Kind) const override {
    // Keep in the same order as enum Fixups in I4004FixupKinds.h.
    // name            offset bits flags
    const static MCFixupKindInfo Infos[I4004::NumTargetFixupKinds] = {
        {"fixup_addr12", 0, 16, 0}, // 12 bits scattered across 2 bytes
        {"fixup_page8", 8, 8, 0},   // byte 1
        {"fixup_imm8", 8, 8, 0},    // byte 1
        {"fixup_fin8", 8, 8, 0},    // byte 1
    };
    static_assert(std::size(Infos) == I4004::NumTargetFixupKinds,
                  "Not all fixup kinds added to Infos array");
    if (mc::isRelocation(Kind))
      return {};
    if (Kind < FirstTargetFixupKind)
      return MCAsmBackend::getFixupKindInfo(Kind);
    return Infos[Kind - FirstTargetFixupKind];
  }

  bool writeNopData(raw_ostream &OS, uint64_t Count,
                    const MCSubtargetInfo *STI) const override {
    // The 4004 NOP is 0x00.
    OS.write_zeros(Count);
    return true;
  }
};

void I4004AsmBackend::applyFixup(const MCFragment &F, const MCFixup &Fixup,
                                 const MCValue &Target, uint8_t *Data,
                                 uint64_t Value, bool IsResolved) {
  // MS2: fixups to symbols are left for the linker (emitted as relocations by
  // the ELF object writer). Record the relocation and, when the value is known
  // (resolved), patch it in.
  maybeAddReloc(F, Fixup, Target, Value, IsResolved);
  if (!IsResolved)
    return;

  unsigned Kind = Fixup.getKind();
  if (Kind < FirstTargetFixupKind)
    return; // data fixups handled by generic code

  switch (Kind) {
  case I4004::fixup_addr12: {
    // A3 -> byte0 low nibble; A2A1 -> byte1.
    Data[0] = (Data[0] & 0xf0) | ((Value >> 8) & 0x0f);
    Data[1] = Value & 0xff;
    break;
  }
  case I4004::fixup_page8:
  case I4004::fixup_imm8:
  case I4004::fixup_fin8:
    Data[1] = Value & 0xff;
    break;
  default:
    llvm_unreachable("unknown I4004 fixup kind");
  }
}

} // end anonymous namespace

MCAsmBackend *llvm::createI4004MCAsmBackend(const Target &T,
                                            const MCSubtargetInfo &STI,
                                            const MCRegisterInfo &MRI,
                                            const MCTargetOptions &Options) {
  return new I4004AsmBackend(STI, ELF::ELFOSABI_STANDALONE);
}
