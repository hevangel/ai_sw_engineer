//===-- I4004MCCodeEmitter.cpp - Convert I4004 code to machine code ------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The I4004MCCodeEmitter turns an MCInst into its 1- or 2-byte MCS-4 encoding.
// Byte 0 is Inst{7-0} (the OPR:OPA opcode byte, emitted first); byte 1 is
// Inst{15-8} (the second program word) for two-byte instructions.
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I4004FixupKinds.h"
#include "MCTargetDesc/I4004MCTargetDesc.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/MC/MCCodeEmitter.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/Support/EndianStream.h"
#include <cstdint>

using namespace llvm;

#define DEBUG_TYPE "mccodeemitter"

namespace llvm {

class I4004MCCodeEmitter : public MCCodeEmitter {
  MCContext &Ctx;
  MCInstrInfo const &MCII;

public:
  I4004MCCodeEmitter(MCContext &ctx, MCInstrInfo const &MCII)
      : Ctx(ctx), MCII(MCII) {}

  // TableGen'erated.
  uint64_t getBinaryCodeForInstr(const MCInst &MI,
                                 SmallVectorImpl<MCFixup> &Fixups,
                                 const MCSubtargetInfo &STI) const;

  // Default operand encoder (registers + plain immediates).
  unsigned getMachineOpValue(const MCInst &MI, const MCOperand &MO,
                             SmallVectorImpl<MCFixup> &Fixups,
                             const MCSubtargetInfo &STI) const;

  // Custom operand encoders for the two-word operands (push fixups for exprs).
  unsigned getImm8OpValue(const MCInst &MI, unsigned OpNo,
                          SmallVectorImpl<MCFixup> &Fixups,
                          const MCSubtargetInfo &STI) const;
  unsigned getPage8OpValue(const MCInst &MI, unsigned OpNo,
                           SmallVectorImpl<MCFixup> &Fixups,
                           const MCSubtargetInfo &STI) const;
  unsigned getAddr12OpValue(const MCInst &MI, unsigned OpNo,
                            SmallVectorImpl<MCFixup> &Fixups,
                            const MCSubtargetInfo &STI) const;

  void encodeInstruction(const MCInst &MI, SmallVectorImpl<char> &CB,
                         SmallVectorImpl<MCFixup> &Fixups,
                         const MCSubtargetInfo &STI) const override;
};

} // end namespace llvm (class decl)

namespace llvm {

void I4004MCCodeEmitter::encodeInstruction(const MCInst &MI,
                                           SmallVectorImpl<char> &CB,
                                           SmallVectorImpl<MCFixup> &Fixups,
                                           const MCSubtargetInfo &STI) const {
  const MCInstrDesc &Desc = MCII.get(MI.getOpcode());
  unsigned Size = Desc.getSize();

  uint64_t Bits = getBinaryCodeForInstr(MI, Fixups, STI);

  // Emit byte 0 (opcode) first, then byte 1 (second word) if present.
  for (unsigned i = 0; i < Size; ++i) {
    CB.push_back((char)((Bits >> (i * 8)) & 0xff));
  }
}

unsigned
I4004MCCodeEmitter::getMachineOpValue(const MCInst &MI, const MCOperand &MO,
                                      SmallVectorImpl<MCFixup> &Fixups,
                                      const MCSubtargetInfo &STI) const {
  if (MO.isReg())
    return Ctx.getRegisterInfo()->getEncodingValue(MO.getReg());
  if (MO.isImm())
    return static_cast<unsigned>(MO.getImm());

  // A bare expr on a non-fixup operand should not happen for MS2 opcodes.
  assert(MO.isExpr() && "unexpected operand kind");
  return 0;
}

// FIM second word: 8-bit immediate (byte offset 1). Expr -> R_I4004_IMM8.
unsigned I4004MCCodeEmitter::getImm8OpValue(const MCInst &MI, unsigned OpNo,
                                            SmallVectorImpl<MCFixup> &Fixups,
                                            const MCSubtargetInfo &STI) const {
  const MCOperand &MO = MI.getOperand(OpNo);
  if (MO.isImm())
    return static_cast<unsigned>(MO.getImm()) & 0xff;
  assert(MO.isExpr() && "expected expr operand");
  Fixups.push_back(MCFixup::create(
      1, MO.getExpr(), MCFixupKind(I4004::fixup_imm8), /*PCRel=*/false));
  return 0;
}

// JCN/ISZ second word: 8-bit page-local address (byte offset 1).
// Expr -> R_I4004_PAGE8.
unsigned I4004MCCodeEmitter::getPage8OpValue(const MCInst &MI, unsigned OpNo,
                                             SmallVectorImpl<MCFixup> &Fixups,
                                             const MCSubtargetInfo &STI) const {
  const MCOperand &MO = MI.getOperand(OpNo);
  if (MO.isImm())
    return static_cast<unsigned>(MO.getImm()) & 0xff;
  assert(MO.isExpr() && "expected expr operand");
  Fixups.push_back(MCFixup::create(
      1, MO.getExpr(), MCFixupKind(I4004::fixup_page8), /*PCRel=*/false));
  return 0;
}

// JUN/JMS: 12-bit absolute address scattered across byte0 low nibble (A3) and
// byte1 (A2A1). Expr -> R_I4004_ADDR12 (fixup covers both bytes, offset 0).
unsigned I4004MCCodeEmitter::getAddr12OpValue(const MCInst &MI, unsigned OpNo,
                                              SmallVectorImpl<MCFixup> &Fixups,
                                              const MCSubtargetInfo &STI) const {
  const MCOperand &MO = MI.getOperand(OpNo);
  if (MO.isImm())
    return static_cast<unsigned>(MO.getImm()) & 0xfff;
  assert(MO.isExpr() && "expected expr operand");
  Fixups.push_back(MCFixup::create(
      0, MO.getExpr(), MCFixupKind(I4004::fixup_addr12), /*PCRel=*/false));
  return 0;
}

MCCodeEmitter *createI4004MCCodeEmitter(const MCInstrInfo &MCII,
                                        MCContext &Ctx) {
  return new I4004MCCodeEmitter(Ctx, MCII);
}

#include "I4004GenMCCodeEmitter.inc"

} // end namespace llvm
