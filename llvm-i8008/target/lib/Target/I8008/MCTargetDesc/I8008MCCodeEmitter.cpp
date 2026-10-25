//===-- I8008MCCodeEmitter.cpp - Convert I8008 code to machine code ------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Emits an MCInst as its 1-, 2- or 3-byte encoding: byte 0 = Inst{7-0}, then
// Inst{15-8}, Inst{23-16} (spec/isa.md; addresses are little-endian).
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8008FixupKinds.h"
#include "MCTargetDesc/I8008MCTargetDesc.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/MC/MCCodeEmitter.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/Support/Casting.h"

using namespace llvm;

#define DEBUG_TYPE "mccodeemitter"

namespace llvm {

class I8008MCCodeEmitter : public MCCodeEmitter {
  MCContext &Ctx;
  MCInstrInfo const &MCII;

public:
  I8008MCCodeEmitter(MCContext &ctx, MCInstrInfo const &MCII)
      : Ctx(ctx), MCII(MCII) {}

  uint64_t getBinaryCodeForInstr(const MCInst &MI,
                                 SmallVectorImpl<MCFixup> &Fixups,
                                 const MCSubtargetInfo &STI) const;

  unsigned getMachineOpValue(const MCInst &MI, const MCOperand &MO,
                             SmallVectorImpl<MCFixup> &Fixups,
                             const MCSubtargetInfo &STI) const {
    if (MO.isReg())
      return Ctx.getRegisterInfo()->getEncodingValue(MO.getReg());
    if (MO.isImm())
      return static_cast<unsigned>(MO.getImm());
    llvm_unreachable("unexpected expression operand");
  }

  // 8-bit immediate in byte 1: a constant, or a fixup for %lo/%hi/a symbol.
  unsigned getImm8OpValue(const MCInst &MI, unsigned OpNo,
                          SmallVectorImpl<MCFixup> &Fixups,
                          const MCSubtargetInfo &STI) const {
    const MCOperand &MO = MI.getOperand(OpNo);
    if (MO.isImm())
      return static_cast<unsigned>(MO.getImm()) & 0xff;
    const MCExpr *Expr = MO.getExpr();
    MCFixupKind Kind = MCFixupKind(I8008::fixup_8);
    if (const auto *SE = dyn_cast<MCSpecifierExpr>(Expr)) {
      if (SE->getSpecifier() == I8008::S_LO)
        Kind = MCFixupKind(I8008::fixup_lo8);
      else if (SE->getSpecifier() == I8008::S_HI)
        Kind = MCFixupKind(I8008::fixup_hi8);
    }
    Fixups.push_back(MCFixup::create(1, Expr, Kind, /*PCRel=*/false));
    return 0;
  }

  // 14-bit address in bytes 1-2.
  unsigned getAddr16OpValue(const MCInst &MI, unsigned OpNo,
                            SmallVectorImpl<MCFixup> &Fixups,
                            const MCSubtargetInfo &STI) const {
    const MCOperand &MO = MI.getOperand(OpNo);
    if (MO.isImm())
      return static_cast<unsigned>(MO.getImm()) & 0x3fff;
    Fixups.push_back(MCFixup::create(1, MO.getExpr(),
                                     MCFixupKind(I8008::fixup_16),
                                     /*PCRel=*/false));
    return 0;
  }

  void encodeInstruction(const MCInst &MI, SmallVectorImpl<char> &CB,
                         SmallVectorImpl<MCFixup> &Fixups,
                         const MCSubtargetInfo &STI) const override {
    unsigned Size = MCII.get(MI.getOpcode()).getSize();
    uint64_t Bits = getBinaryCodeForInstr(MI, Fixups, STI);
    for (unsigned I = 0; I < Size; ++I)
      CB.push_back(static_cast<char>((Bits >> (I * 8)) & 0xff));
  }
};

MCCodeEmitter *createI8008MCCodeEmitter(const MCInstrInfo &MCII,
                                        MCContext &Ctx) {
  return new I8008MCCodeEmitter(Ctx, MCII);
}

#include "I8008GenMCCodeEmitter.inc"

} // end namespace llvm
