//===-- I8086AsmBackend.cpp - I8086 Assembler Backend ---------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Fixups, no-ops and branch relaxation. A bare "jmp L" starts as the two-byte
// short form and grows to the three-byte near form; a bare "jcc L" grows to the
// five-byte "j!cc +3 ; jmp near L" because conditional jumps have no near form.
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8086FixupKinds.h"
#include "MCTargetDesc/I8086MCTargetDesc.h"
#include "llvm/ADT/StringSwitch.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/MC/MCAsmBackend.h"
#include "llvm/MC/MCAssembler.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCELFObjectWriter.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCObjectWriter.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/MCValue.h"
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;

namespace {

/// The relaxed opcode of a relaxable branch, or 0.
unsigned relaxedOpcode(unsigned Opcode) {
  switch (Opcode) {
  case I8086::JMP_R:
    return I8086::JMP_N;
#define JCC(N)                                                                 \
  case I8086::N##_R:                                                           \
    return I8086::N##_F;
    JCC(JO) JCC(JNO) JCC(JB) JCC(JAE) JCC(JE) JCC(JNE) JCC(JBE) JCC(JA)
    JCC(JS) JCC(JNS) JCC(JP) JCC(JNP) JCC(JL) JCC(JGE) JCC(JLE) JCC(JG)
#undef JCC
  default:
    return 0;
  }
}

class I8086AsmBackend : public MCAsmBackend {
  const MCSubtargetInfo &STI;
  uint8_t OSABI;

public:
  I8086AsmBackend(const MCSubtargetInfo &STI, uint8_t OSABI)
      : MCAsmBackend(llvm::endianness::little), STI(STI), OSABI(OSABI) {}

  void applyFixup(const MCFragment &, const MCFixup &, const MCValue &Target,
                  uint8_t *Data, uint64_t Value, bool IsResolved) override;

  std::unique_ptr<MCObjectTargetWriter>
  createObjectTargetWriter() const override {
    return createI8086ELFObjectWriter(OSABI);
  }

  std::optional<MCFixupKind> getFixupKind(StringRef Name) const override {
    unsigned Type = StringSwitch<unsigned>(Name)
#define ELF_RELOC(NAME, ID) .Case(#NAME, ID)
#include "llvm/BinaryFormat/ELFRelocs/I8086.def"
#undef ELF_RELOC
                        .Case("BFD_RELOC_NONE", ELF::R_I8086_NONE)
                        .Default(-1u);
    if (Type != -1u)
      return static_cast<MCFixupKind>(FirstLiteralRelocationKind + Type);
    return std::nullopt;
  }

  MCFixupKindInfo getFixupKindInfo(MCFixupKind Kind) const override {
    const static MCFixupKindInfo Infos[I8086::NumTargetFixupKinds] = {
        {"fixup_8", 0, 8, 0},
        {"fixup_16", 0, 16, 0},
        {"fixup_pc8", 0, 8, 0},
        {"fixup_pc16", 0, 16, 0},
    };
    if (mc::isRelocation(Kind))
      return {};
    if (Kind < FirstTargetFixupKind)
      return MCAsmBackend::getFixupKindInfo(Kind);
    return Infos[Kind - FirstTargetFixupKind];
  }

  bool mayNeedRelaxation(unsigned Opcode, ArrayRef<MCOperand> Operands,
                         const MCSubtargetInfo &STI) const override {
    return relaxedOpcode(Opcode) != 0;
  }

  bool fixupNeedsRelaxationAdvanced(const MCFragment &, const MCFixup &Fixup,
                                    const MCValue &, uint64_t Value,
                                    bool Resolved) const override {
    if (Fixup.getKind() != I8086::fixup_pc8)
      return false;
    // A symbol the linker places must use the form with a 16-bit displacement.
    if (!Resolved)
      return true;
    int64_t V = int64_t(Value);
    return V < -128 || V > 127;
  }

  void relaxInstruction(MCInst &Inst,
                        const MCSubtargetInfo &STI) const override {
    unsigned Op = relaxedOpcode(Inst.getOpcode());
    assert(Op && "only relaxable branches are relaxed");
    Inst.setOpcode(Op);
  }

  bool writeNopData(raw_ostream &OS, uint64_t Count,
                    const MCSubtargetInfo *STI) const override {
    for (uint64_t I = 0; I < Count; ++I)
      OS << char(0x90);
    return true;
  }
};

void I8086AsmBackend::applyFixup(const MCFragment &F, const MCFixup &Fixup,
                                 const MCValue &Target, uint8_t *Data,
                                 uint64_t Value, bool IsResolved) {
  maybeAddReloc(F, Fixup, Target, Value, IsResolved);
  if (!IsResolved)
    return;
  switch (unsigned Kind = Fixup.getKind()) {
  case FK_Data_1:
  case I8086::fixup_8:
    Data[0] = Value & 0xff;
    break;
  case I8086::fixup_pc8: {
    int64_t V = int64_t(Value);
    if (V < -128 || V > 127) {
      getContext().reportError(Fixup.getLoc(),
                               "branch target out of range for a short jump");
      return;
    }
    Data[0] = Value & 0xff;
    break;
  }
  case FK_Data_2:
  case I8086::fixup_16:
  case I8086::fixup_pc16:
    Data[0] = Value & 0xff;
    Data[1] = (Value >> 8) & 0xff;
    break;
  default:
    if (Kind >= FirstTargetFixupKind)
      llvm_unreachable("unknown I8086 fixup kind");
    break;
  }
}

} // end anonymous namespace

MCAsmBackend *llvm::createI8086MCAsmBackend(const Target &T,
                                            const MCSubtargetInfo &STI,
                                            const MCRegisterInfo &MRI,
                                            const MCTargetOptions &Options) {
  return new I8086AsmBackend(STI, ELF::ELFOSABI_STANDALONE);
}
