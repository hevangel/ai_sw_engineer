//===-- I8086MCCodeEmitter.cpp - Convert I8086 code to machine code -------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Emits an MCInst as [segment prefix] opcode [ModRM [displacement]] [immediate],
// interpreting the TSFlags of I8086InstrFormats.td (spec/isa.md).
//
// Operand layout of an MCInst: a register is one operand, a memory reference
// is four (base, index, displacement, segment), a far pointer is two (offset,
// segment). For the ModRM forms the register-field operand and the r/m operand
// are in the order given by the RegFirst attribute; the immediate follows.
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8086BaseInfo.h"
#include "MCTargetDesc/I8086FixupKinds.h"
#include "MCTargetDesc/I8086MCAsmInfo.h"
#include "MCTargetDesc/I8086MCTargetDesc.h"
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
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;
using namespace llvm::I8086II;

#define DEBUG_TYPE "mccodeemitter"

namespace {

class I8086MCCodeEmitter : public MCCodeEmitter {
  MCContext &Ctx;
  const MCInstrInfo &MCII;

  unsigned enc(MCRegister R) const {
    return Ctx.getRegisterInfo()->getEncodingValue(R);
  }

  void emitByte(SmallVectorImpl<char> &CB, unsigned B) const {
    CB.push_back(static_cast<char>(B & 0xff));
  }

  /// A constant of Size bytes, or zeros and a fixup for a relocatable value.
  void emitValue(const MCOperand &Op, unsigned Size, SmallVectorImpl<char> &CB,
                 SmallVectorImpl<MCFixup> &Fixups, bool PCRel = false,
                 int64_t Bias = 0) const;

  /// The first operand at or after From that is an immediate or an expression.
  unsigned findImm(const MCInst &MI, unsigned From) const {
    for (unsigned I = From, E = MI.getNumOperands(); I != E; ++I)
      if (MI.getOperand(I).isImm() || MI.getOperand(I).isExpr())
        return I;
    llvm_unreachable("instruction has no immediate operand");
  }

  /// Segment override prefix for the four-operand memory reference at Mem.
  void emitSegPrefix(const MCInst &MI, unsigned Mem,
                     SmallVectorImpl<char> &CB) const {
    MCRegister Seg = MI.getOperand(Mem + 3).getReg();
    if (Seg)
      emitByte(CB, 0x26 + (enc(Seg) << 3));
  }

  void emitMemModRM(const MCInst &MI, unsigned Mem, unsigned RegField,
                    SmallVectorImpl<char> &CB,
                    SmallVectorImpl<MCFixup> &Fixups) const;

public:
  I8086MCCodeEmitter(MCContext &ctx, const MCInstrInfo &MCII)
      : Ctx(ctx), MCII(MCII) {}

  void encodeInstruction(const MCInst &MI, SmallVectorImpl<char> &CB,
                         SmallVectorImpl<MCFixup> &Fixups,
                         const MCSubtargetInfo &STI) const override;
};

void I8086MCCodeEmitter::emitValue(const MCOperand &Op, unsigned Size,
                                   SmallVectorImpl<char> &CB,
                                   SmallVectorImpl<MCFixup> &Fixups,
                                   bool PCRel, int64_t Bias) const {
  int64_t V = 0;
  const MCExpr *Expr = nullptr;
  if (Op.isImm()) {
    V = Op.getImm();
    if (PCRel)
      Expr = MCConstantExpr::create(V, Ctx);
  } else {
    Expr = Op.getExpr();
    if (const auto *CE = dyn_cast<MCConstantExpr>(Expr)) {
      V = CE->getValue();
      if (!PCRel)
        Expr = nullptr;
    }
  }
  if (Expr && (PCRel || !isa<MCConstantExpr>(Expr))) {
    MCFixupKind Kind;
    const auto *Ref = dyn_cast<MCSymbolRefExpr>(Expr);
    if (Ref && Ref->getSpecifier() == I8086::S_SEG && !PCRel && Size == 2) {
      // The segment of the symbol: a relocation against the plain symbol.
      Expr = MCSymbolRefExpr::create(&Ref->getSymbol(), Ctx);
      Kind = MCFixupKind(I8086::fixup_seg16);
    } else if (PCRel) {
      Kind = MCFixupKind(Size == 1 ? I8086::fixup_pc8 : I8086::fixup_pc16);
      Expr = MCBinaryExpr::createAdd(Expr, MCConstantExpr::create(Bias, Ctx),
                                     Ctx);
    } else {
      Kind = MCFixupKind(Size == 1 ? I8086::fixup_8 : I8086::fixup_16);
    }
    Fixups.push_back(MCFixup::create(CB.size(), Expr, Kind, PCRel));
    V = 0;
  }
  for (unsigned I = 0; I < Size; ++I)
    emitByte(CB, V >> (8 * I));
}

void I8086MCCodeEmitter::emitMemModRM(const MCInst &MI, unsigned Mem,
                                      unsigned RegField,
                                      SmallVectorImpl<char> &CB,
                                      SmallVectorImpl<MCFixup> &Fixups) const {
  MCRegister Base = MI.getOperand(Mem).getReg();
  MCRegister Index = MI.getOperand(Mem + 1).getReg();
  const MCOperand &Disp = MI.getOperand(Mem + 2);

  // Register combinations the hardware encodes in the r/m field.
  int RM;
  if (!Base && !Index)
    RM = 6; // direct address (mod 00)
  else if (Base == I8086::BX && Index == I8086::SI)
    RM = 0;
  else if (Base == I8086::BX && Index == I8086::DI)
    RM = 1;
  else if (Base == I8086::BP && Index == I8086::SI)
    RM = 2;
  else if (Base == I8086::BP && Index == I8086::DI)
    RM = 3;
  else if (!Base && Index == I8086::SI)
    RM = 4;
  else if (!Base && Index == I8086::DI)
    RM = 5;
  else if (Base == I8086::BP && !Index)
    RM = 6;
  else if (Base == I8086::BX && !Index)
    RM = 7;
  else
    llvm_unreachable("invalid 16-bit addressing mode");

  bool Direct = !Base && !Index;
  bool Constant = Disp.isImm() ||
                  (Disp.isExpr() && isa<MCConstantExpr>(Disp.getExpr()));
  int64_t V = 0;
  if (Disp.isImm())
    V = Disp.getImm();
  else if (Constant)
    V = cast<MCConstantExpr>(Disp.getExpr())->getValue();
  V = static_cast<int16_t>(V & 0xffff);

  unsigned Mod;
  unsigned DispSize;
  if (Direct) {
    Mod = 0;
    DispSize = 2;
  } else if (!Constant) {
    Mod = 2;
    DispSize = 2;
  } else if (V == 0 && RM != 6) {
    Mod = 0;
    DispSize = 0;
  } else if (V >= -128 && V <= 127) {
    Mod = 1;
    DispSize = 1;
  } else {
    Mod = 2;
    DispSize = 2;
  }
  emitByte(CB, (Mod << 6) | (RegField << 3) | RM);
  if (DispSize)
    emitValue(Disp, DispSize, CB, Fixups);
}

void I8086MCCodeEmitter::encodeInstruction(const MCInst &MI,
                                           SmallVectorImpl<char> &CB,
                                           SmallVectorImpl<MCFixup> &Fixups,
                                           const MCSubtargetInfo &STI) const {
  const uint64_t TS = MCII.get(MI.getOpcode()).TSFlags;
  const unsigned Opc = opcode(TS);

  switch (form(TS)) {
  case FormNone: {
    emitByte(CB, Opc);
    if (hasFixed(TS))
      emitByte(CB, fixedByte(TS));
    if (imm(TS) != ImmNone)
      emitValue(MI.getOperand(findImm(MI, 0)), imm(TS) == Imm8 ? 1 : 2, CB,
                Fixups);
    break;
  }
  case FormRegOpc: {
    unsigned RegIdx = regOp1(TS) ? 1 : 0;
    emitByte(CB, Opc | enc(MI.getOperand(RegIdx).getReg()));
    if (imm(TS) != ImmNone)
      emitValue(MI.getOperand(findImm(MI, 0)), imm(TS) == Imm8 ? 1 : 2, CB,
                Fixups);
    break;
  }
  case FormSegOpc:
    emitByte(CB, Opc | (enc(MI.getOperand(0).getReg()) << 3));
    break;
  case FormModRM:
  case FormDigit:
  case FormModRMSeg: {
    const bool Reg = rmReg(TS);
    const unsigned RMSize = Reg ? 1 : 4;
    unsigned RMStart = 0, RegOp = 0, End;
    if (form(TS) == FormDigit) {
      End = RMSize;
    } else if (regFirst(TS)) {
      RegOp = 0;
      RMStart = 1;
      End = 1 + RMSize;
    } else {
      RMStart = 0;
      RegOp = RMSize;
      End = RMSize + 1;
    }
    if (!Reg)
      emitSegPrefix(MI, RMStart, CB);
    emitByte(CB, Opc);
    unsigned Field = form(TS) == FormDigit
                         ? digit(TS)
                         : enc(MI.getOperand(RegOp).getReg());
    if (Reg)
      emitByte(CB, 0xC0 | (Field << 3) | enc(MI.getOperand(RMStart).getReg()));
    else
      emitMemModRM(MI, RMStart, Field, CB, Fixups);
    if (imm(TS) != ImmNone)
      emitValue(MI.getOperand(findImm(MI, End)), imm(TS) == Imm8 ? 1 : 2, CB,
                Fixups);
    break;
  }
  case FormRel8:
    emitByte(CB, Opc);
    emitValue(MI.getOperand(0), 1, CB, Fixups, /*PCRel=*/true, /*Bias=*/-1);
    break;
  case FormRel16:
    emitByte(CB, Opc);
    emitValue(MI.getOperand(0), 2, CB, Fixups, /*PCRel=*/true, /*Bias=*/-2);
    break;
  case FormJccFar:
    // The inverted condition skips the three-byte near jump that follows.
    emitByte(CB, Opc ^ 1);
    emitByte(CB, 3);
    emitByte(CB, 0xE9);
    emitValue(MI.getOperand(0), 2, CB, Fixups, /*PCRel=*/true, /*Bias=*/-2);
    break;
  case FormFar:
    emitByte(CB, Opc);
    emitValue(MI.getOperand(0), 2, CB, Fixups);
    emitValue(MI.getOperand(1), 2, CB, Fixups);
    break;
  case FormMoffs: {
    // A0/A1 load (AL/AX, memory); A2/A3 store (memory, AL/AX).
    unsigned Mem = Opc < 0xA2 ? 1 : 0;
    emitSegPrefix(MI, Mem, CB);
    emitByte(CB, Opc);
    emitValue(MI.getOperand(Mem + 2), 2, CB, Fixups);
    break;
  }
  }
}

} // end anonymous namespace

MCCodeEmitter *llvm::createI8086MCCodeEmitter(const MCInstrInfo &MCII,
                                              MCContext &Ctx) {
  return new I8086MCCodeEmitter(Ctx, MCII);
}
