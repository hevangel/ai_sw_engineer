//===-- I4004Disassembler.cpp - Disassembler for I4004 ------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The MCS-4 instruction stream is variable length (1 or 2 program bytes) and
// its opcode map is small and fully regular, so this is a hand-written decoder
// keyed off the high nibble OPR of byte 0, with the low nibble OPA selecting a
// register, a pair, an immediate or a sub-op. Every case below is the direct
// transcription of one row of llvm-i4004/spec/isa.md; TableGen's fixed-length
// decoder (-gen-disassembler) assumes a single instruction width and is a poor
// fit for the 1/2-byte mix, so it is deliberately not used.
//
// Two-byte instructions (JCN, FIM, JUN, JMS, ISZ) read a second program byte;
// if it is not available the decoder returns Fail with Size = 1 so the caller
// can resynchronise. Undefined accumulator opcodes 0xFE/0xFF decode to Fail
// (isa.md §8: a disassembler may render them as .byte).
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I4004MCTargetDesc.h"
#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCDecoderOps.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-disassembler"

typedef MCDisassembler::DecodeStatus DecodeStatus;

namespace {

class I4004Disassembler : public MCDisassembler {
public:
  I4004Disassembler(const MCSubtargetInfo &STI, MCContext &Ctx)
      : MCDisassembler(STI, Ctx) {}
  ~I4004Disassembler() override = default;

  DecodeStatus getInstruction(MCInst &Instr, uint64_t &Size,
                              ArrayRef<uint8_t> Bytes, uint64_t Address,
                              raw_ostream &CStream) const override;
};

} // end anonymous namespace

// R0..R15 are the first 16 physical registers in GR4; P0..P7 in PR8. The MC
// register-info init order gives contiguous ids, but to stay robust we look
// them up through the generated enum.
static MCRegister getGR4Reg(unsigned Idx) {
  static const MCPhysReg GR4[] = {
      I4004::R0,  I4004::R1,  I4004::R2,  I4004::R3,
      I4004::R4,  I4004::R5,  I4004::R6,  I4004::R7,
      I4004::R8,  I4004::R9,  I4004::R10, I4004::R11,
      I4004::R12, I4004::R13, I4004::R14, I4004::R15};
  return GR4[Idx & 0xf];
}

static MCRegister getPR8Reg(unsigned Idx) {
  static const MCPhysReg PR8[] = {I4004::P0, I4004::P1, I4004::P2, I4004::P3,
                                  I4004::P4, I4004::P5, I4004::P6, I4004::P7};
  return PR8[Idx & 0x7];
}

DecodeStatus I4004Disassembler::getInstruction(MCInst &MI, uint64_t &Size,
                                               ArrayRef<uint8_t> Bytes,
                                               uint64_t Address,
                                               raw_ostream &CS) const {
  if (Bytes.size() < 1) {
    Size = 0;
    return MCDisassembler::Fail;
  }

  uint8_t B0 = Bytes[0];
  uint8_t OPR = (B0 >> 4) & 0xf;
  uint8_t OPA = B0 & 0xf;

  // Helper: fetch the second program byte for a two-word instruction.
  auto need2 = [&](uint8_t &W2) -> bool {
    if (Bytes.size() < 2) {
      Size = 1; // consumed the opcode byte; caller resynchronises
      return false;
    }
    W2 = Bytes[1];
    return true;
  };

  Size = 1; // default; two-word cases bump to 2

  switch (OPR) {
  case 0x0: // 0000 ....
    // Only 0x00 is NOP; 0x01..0x0F are not defined MCS-4 opcodes.
    if (OPA != 0x0)
      return MCDisassembler::Fail;
    MI.setOpcode(I4004::NOP);
    return MCDisassembler::Success;

  case 0x1: { // JCN cond, addr8 (two words)
    uint8_t W2;
    if (!need2(W2))
      return MCDisassembler::Fail;
    MI.setOpcode(I4004::JCN);
    MI.addOperand(MCOperand::createImm(OPA)); // condition nibble
    MI.addOperand(MCOperand::createImm(W2));  // page-local address
    Size = 2;
    return MCDisassembler::Success;
  }

  case 0x2: // 0010 RRRb: b=0 FIM (two words), b=1 SRC (one word)
    if ((OPA & 0x1) == 0) {
      uint8_t W2;
      if (!need2(W2))
        return MCDisassembler::Fail;
      MI.setOpcode(I4004::FIM);
      MI.addOperand(MCOperand::createReg(getPR8Reg(OPA >> 1)));
      MI.addOperand(MCOperand::createImm(W2));
      Size = 2;
      return MCDisassembler::Success;
    }
    MI.setOpcode(I4004::SRC);
    MI.addOperand(MCOperand::createReg(getPR8Reg(OPA >> 1)));
    return MCDisassembler::Success;

  case 0x3: // 0011 RRRb: b=0 FIN, b=1 JIN (both one word)
    if ((OPA & 0x1) == 0)
      MI.setOpcode(I4004::FIN);
    else
      MI.setOpcode(I4004::JIN);
    MI.addOperand(MCOperand::createReg(getPR8Reg(OPA >> 1)));
    return MCDisassembler::Success;

  case 0x4:   // JUN addr12 (two words)
  case 0x5: { // JMS addr12 (two words)
    uint8_t W2;
    if (!need2(W2))
      return MCDisassembler::Fail;
    MI.setOpcode(OPR == 0x4 ? I4004::JUN : I4004::JMS);
    unsigned Addr12 = (unsigned(OPA) << 8) | W2; // A3 in OPA, A2A1 in W2
    MI.addOperand(MCOperand::createImm(Addr12));
    Size = 2;
    return MCDisassembler::Success;
  }

  case 0x6: // INC Rn
    MI.setOpcode(I4004::INC);
    MI.addOperand(MCOperand::createReg(getGR4Reg(OPA)));
    return MCDisassembler::Success;

  case 0x7: { // ISZ Rn, addr8 (two words)
    uint8_t W2;
    if (!need2(W2))
      return MCDisassembler::Fail;
    MI.setOpcode(I4004::ISZ);
    MI.addOperand(MCOperand::createReg(getGR4Reg(OPA)));
    MI.addOperand(MCOperand::createImm(W2));
    Size = 2;
    return MCDisassembler::Success;
  }

  case 0x8: // ADD Rn
  case 0x9: // SUB Rn
  case 0xA: // LD Rn
  case 0xB: // XCH Rn
  {
    static const unsigned Ops[] = {I4004::ADD, I4004::SUB, I4004::LD,
                                   I4004::XCH};
    MI.setOpcode(Ops[OPR - 0x8]);
    MI.addOperand(MCOperand::createReg(getGR4Reg(OPA)));
    return MCDisassembler::Success;
  }

  case 0xC: // BBL imm4
    MI.setOpcode(I4004::BBL);
    MI.addOperand(MCOperand::createImm(OPA));
    return MCDisassembler::Success;

  case 0xD: // LDM imm4
    MI.setOpcode(I4004::LDM);
    MI.addOperand(MCOperand::createImm(OPA));
    return MCDisassembler::Success;

  case 0xE: { // RAM/ROM port group, OPA selects the op (all one word)
    static const unsigned Ops[16] = {
        I4004::WRM, I4004::WMP, I4004::WRR, I4004::WPM,
        I4004::WR0, I4004::WR1, I4004::WR2, I4004::WR3,
        I4004::SBM, I4004::RDM, I4004::RDR, I4004::ADM,
        I4004::RD0, I4004::RD1, I4004::RD2, I4004::RD3};
    MI.setOpcode(Ops[OPA]);
    return MCDisassembler::Success;
  }

  case 0xF: { // Accumulator group; 0xFE/0xFF undefined
    static const unsigned Ops[16] = {
        I4004::CLB, I4004::CLC, I4004::IAC, I4004::CMC,
        I4004::CMA, I4004::RAL, I4004::RAR, I4004::TCC,
        I4004::DAC, I4004::TCS, I4004::STC, I4004::DAA,
        I4004::KBP, I4004::DCL, 0 /*undef*/, 0 /*undef*/};
    if (OPA >= 0xE)
      return MCDisassembler::Fail; // isa.md §8: undefined -> .byte
    MI.setOpcode(Ops[OPA]);
    return MCDisassembler::Success;
  }
  }

  return MCDisassembler::Fail;
}

static MCDisassembler *createI4004Disassembler(const Target &T,
                                               const MCSubtargetInfo &STI,
                                               MCContext &Ctx) {
  return new I4004Disassembler(STI, Ctx);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI4004Disassembler() {
  TargetRegistry::RegisterMCDisassembler(getTheI4004Target(),
                                         createI4004Disassembler);
}
