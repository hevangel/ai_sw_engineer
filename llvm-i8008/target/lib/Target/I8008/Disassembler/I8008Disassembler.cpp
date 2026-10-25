//===-- I8008Disassembler.cpp - Disassembler for I8008 ------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Hand-written decoder keyed on the octal fields of the opcode byte
// (spec/isa.md): bits 7-6 select the group, bits 5-3 and 2-0 the registers,
// the ALU operation or the condition. Undefined opcodes decode to Fail (the
// caller prints them as .byte).
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8008MCTargetDesc.h"
#include "MCTargetDesc/I8008OpTables.h"
#include "TargetInfo/I8008TargetInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCDecoderOps.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define DEBUG_TYPE "i8008-disassembler"

typedef MCDisassembler::DecodeStatus DecodeStatus;

namespace {
class I8008Disassembler : public MCDisassembler {
public:
  I8008Disassembler(const MCSubtargetInfo &STI, MCContext &Ctx)
      : MCDisassembler(STI, Ctx) {}

  DecodeStatus getInstruction(MCInst &MI, uint64_t &Size,
                              ArrayRef<uint8_t> Bytes, uint64_t Address,
                              raw_ostream &CS) const override;
};
} // end anonymous namespace

DecodeStatus I8008Disassembler::getInstruction(MCInst &MI, uint64_t &Size,
                                               ArrayRef<uint8_t> Bytes,
                                               uint64_t Address,
                                               raw_ostream &CS) const {
  using namespace I8008Op;
  Size = 1;
  if (Bytes.empty()) {
    Size = 0;
    return MCDisassembler::Fail;
  }
  const unsigned B0 = Bytes[0];
  const unsigned G = B0 >> 6, X = (B0 >> 3) & 7, Y = B0 & 7;

  auto Imm8 = [&](unsigned Opc) {
    if (Bytes.size() < 2)
      return MCDisassembler::Fail;
    Size = 2;
    MI.setOpcode(Opc);
    MI.addOperand(MCOperand::createImm(Bytes[1]));
    return MCDisassembler::Success;
  };
  auto Addr = [&](unsigned Opc) {
    if (Bytes.size() < 3)
      return MCDisassembler::Fail;
    Size = 3;
    MI.setOpcode(Opc);
    MI.addOperand(MCOperand::createImm(Bytes[1] | ((Bytes[2] & 0x3f) << 8)));
    return MCDisassembler::Success;
  };
  auto One = [&](unsigned Opc) {
    MI.setOpcode(Opc);
    return MCDisassembler::Success;
  };

  switch (G) {
  case 3: // 11ddd sss: loads
    return One(MOV[X][Y]);
  case 2: // 10ooo sss: ALU
    return One(ALU[X][Y]);
  case 0:
    switch (Y) {
    case 0:
      return X == 0 ? One(I8008::HLT) : (X == 7 ? MCDisassembler::Fail
                                                 : One(INR[X]));
    case 1:
      return X == 0 ? One(I8008::HLT) : (X == 7 ? MCDisassembler::Fail
                                                 : One(DCR[X]));
    case 2: {
      static const unsigned Rot[4] = {I8008::RLC, I8008::RRC, I8008::RAL,
                                      I8008::RAR};
      return X < 4 ? One(Rot[X]) : MCDisassembler::Fail;
    }
    case 3:
      return One(RCC[X >> 2][X & 3]);
    case 4:
      return Imm8(ALUI[X]);
    case 5:
      MI.setOpcode(I8008::RST);
      MI.addOperand(MCOperand::createImm(X));
      return MCDisassembler::Success;
    case 6:
      return Imm8(MVI[X]);
    default: // 7
      return One(I8008::RET);
    }
  default: // 01xxx xxx
    if (B0 & 1) {
      unsigned Port = (B0 >> 1) & 0x1f;
      MI.setOpcode(Port < 8 ? I8008::INP : I8008::OUT);
      MI.addOperand(MCOperand::createImm(Port));
      return MCDisassembler::Success;
    }
    switch (Y) {
    case 0:
      return Addr(JCC[X >> 2][X & 3]);
    case 2:
      return Addr(CCC[X >> 2][X & 3]);
    case 4:
      return Addr(I8008::JMP);
    case 6:
      return Addr(I8008::CAL);
    default:
      return MCDisassembler::Fail;
    }
  }
}

static MCDisassembler *createI8008Disassembler(const Target &T,
                                               const MCSubtargetInfo &STI,
                                               MCContext &Ctx) {
  return new I8008Disassembler(STI, Ctx);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8008Disassembler() {
  TargetRegistry::RegisterMCDisassembler(getTheI8008Target(),
                                         createI8008Disassembler);
}
