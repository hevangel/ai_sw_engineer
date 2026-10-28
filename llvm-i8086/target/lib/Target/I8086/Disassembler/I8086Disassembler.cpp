//===-- I8086Disassembler.cpp - Disassembler for I8086 ---------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Table-driven from the same TSFlags the code emitter reads (I8086InstrFormats
// .td): the decoder indexes every decodable instruction by opcode byte, picks
// the one whose ModRM mod, /digit and fixed byte match, and builds the operand
// list in the order the emitter expects. Undefined opcodes decode to Fail (the
// caller prints them as .byte). A segment-override prefix in front of a
// memory-operand instruction is folded into the operand; every other prefix is
// an instruction of its own.
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8086BaseInfo.h"
#include "MCTargetDesc/I8086MCTargetDesc.h"
#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCDecoderOps.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;
using namespace llvm::I8086II;

#define DEBUG_TYPE "i8086-disassembler"

typedef MCDisassembler::DecodeStatus DecodeStatus;

namespace {

class I8086Disassembler : public MCDisassembler {
  std::unique_ptr<MCInstrInfo> MCII;
  /// Decodable instructions by first opcode byte, exact-byte forms first.
  std::vector<unsigned> Table[256];

  MCRegister regFor(int RC, unsigned N, bool &Ok) const;
  bool decodeOne(MCInst &MI, uint64_t &Size, ArrayRef<uint8_t> Bytes,
                 uint64_t Address, int &MemIdx) const;
  bool modrmOperands(MCInst &MI, ArrayRef<uint8_t> Bytes, unsigned &Pos,
                     const MCInstrDesc &Desc, uint64_t TS, int &MemIdx) const;

public:
  I8086Disassembler(const MCSubtargetInfo &STI, MCContext &Ctx,
                    std::unique_ptr<MCInstrInfo> II);

  DecodeStatus getInstruction(MCInst &MI, uint64_t &Size,
                              ArrayRef<uint8_t> Bytes, uint64_t Address,
                              raw_ostream &CS) const override;
};

I8086Disassembler::I8086Disassembler(const MCSubtargetInfo &STI, MCContext &Ctx,
                                     std::unique_ptr<MCInstrInfo> II)
    : MCDisassembler(STI, Ctx), MCII(std::move(II)) {
  std::vector<unsigned> Exact[256], RegOpc[256];
  for (unsigned Op = 0, E = MCII->getNumOpcodes(); Op != E; ++Op) {
    uint64_t TS = MCII->get(Op).TSFlags;
    // Only instructions with an encoding (the TSFlags form is set for all of
    // them; pseudo-instructions have none).
    if (MCII->get(Op).isPseudo() || noDecode(TS) || form(TS) == FormJccFar)
      continue;
    unsigned Opc = opcode(TS);
    switch (form(TS)) {
    case FormRegOpc:
      for (unsigned R = 0; R < 8; ++R)
        RegOpc[Opc | R].push_back(Op);
      break;
    case FormSegOpc:
      for (unsigned S = 0; S < 4; ++S)
        Exact[Opc | (S << 3)].push_back(Op);
      break;
    default:
      // Instructions with a fixed second byte first, so AAM beats AAM imm.
      if (hasFixed(TS))
        Exact[Opc].insert(Exact[Opc].begin(), Op);
      else
        Exact[Opc].push_back(Op);
      break;
    }
  }
  for (unsigned B = 0; B < 256; ++B) {
    Table[B] = Exact[B];
    Table[B].insert(Table[B].end(), RegOpc[B].begin(), RegOpc[B].end());
  }
}

/// Register number N (an encoding field value) in register class RC. A
/// single-register class names its register regardless of N.
MCRegister I8086Disassembler::regFor(int RC, unsigned N, bool &Ok) const {
  const MCRegisterInfo &MRI = *getContext().getRegisterInfo();
  const MCRegisterClass &Class = MRI.getRegClass(RC);
  if (Class.getNumRegs() == 1)
    return Class.getRegister(0);
  if (RC == I8086::SEGREGNCRegClassID) {
    if (N == 1 || N > 3) {
      Ok = false;
      return MCRegister();
    }
    return MRI.getRegClass(I8086::SEGREGRegClassID).getRegister(N);
  }
  if (N >= Class.getNumRegs()) {
    Ok = false;
    return MCRegister();
  }
  return Class.getRegister(N);
}

static MCRegister segmentRegister(unsigned N) {
  static const MCRegister Segs[4] = {I8086::ES, I8086::CS, I8086::SS,
                                     I8086::DS};
  return Segs[N & 3];
}

/// Append the (register or four-operand memory) r/m operand and the reg-field
/// operand of a ModRM instruction in emitter order. Pos is the index of the
/// ModRM byte and is advanced past the displacement.
bool I8086Disassembler::modrmOperands(MCInst &MI, ArrayRef<uint8_t> Bytes,
                                      unsigned &Pos, const MCInstrDesc &Desc,
                                      uint64_t TS, int &MemIdx) const {
  const unsigned M = Bytes[Pos++];
  const unsigned Mod = M >> 6, Field = (M >> 3) & 7, RM = M & 7;
  const bool Reg = rmReg(TS);
  const Form F = form(TS);
  auto Infos = Desc.operands();
  const unsigned RMSize = Reg ? 1 : 4;
  const unsigned RMStart = (F != FormDigit && regFirst(TS)) ? 1 : 0;
  const unsigned RegSlot = regFirst(TS) ? 0 : RMSize;
  bool Ok = true;

  // The register-field operand.
  MCOperand FieldOp;
  if (F != FormDigit) {
    int RC = Infos[RegSlot].RegClass;
    FieldOp = MCOperand::createReg(regFor(RC, Field, Ok));
  }
  // The r/m operand.
  SmallVector<MCOperand, 4> RMOps;
  if (Reg) {
    RMOps.push_back(MCOperand::createReg(regFor(Infos[RMStart].RegClass, RM, Ok)));
  } else {
    MCRegister Base, Index;
    int64_t Disp = 0;
    unsigned DispSize = 0;
    if (Mod == 0 && RM == 6) {
      DispSize = 2;
    } else {
      static const MCRegister Bases[8] = {I8086::BX, I8086::BX, I8086::BP,
                                          I8086::BP, MCRegister(), MCRegister(),
                                          I8086::BP, I8086::BX};
      static const MCRegister Indexes[8] = {I8086::SI, I8086::DI, I8086::SI,
                                            I8086::DI, I8086::SI, I8086::DI,
                                            MCRegister(), MCRegister()};
      Base = Bases[RM];
      Index = Indexes[RM];
      DispSize = Mod == 1 ? 1 : Mod == 2 ? 2 : 0;
    }
    if (Pos + DispSize > Bytes.size())
      return false;
    if (DispSize == 1)
      Disp = static_cast<int8_t>(Bytes[Pos]);
    else if (DispSize == 2)
      Disp = static_cast<int16_t>(Bytes[Pos] | (Bytes[Pos + 1] << 8));
    Pos += DispSize;
    RMOps.push_back(MCOperand::createReg(Base));
    RMOps.push_back(MCOperand::createReg(Index));
    RMOps.push_back(MCOperand::createImm(Disp));
    RMOps.push_back(MCOperand::createReg(MCRegister()));
  }
  if (!Ok)
    return false;

  if (F == FormDigit) {
    MemIdx = Reg ? -1 : 0;
    for (const MCOperand &O : RMOps)
      MI.addOperand(O);
  } else if (regFirst(TS)) {
    MI.addOperand(FieldOp);
    MemIdx = Reg ? -1 : 1;
    for (const MCOperand &O : RMOps)
      MI.addOperand(O);
  } else {
    MemIdx = Reg ? -1 : 0;
    for (const MCOperand &O : RMOps)
      MI.addOperand(O);
    MI.addOperand(FieldOp);
  }
  return true;
}

bool I8086Disassembler::decodeOne(MCInst &MI, uint64_t &Size,
                                  ArrayRef<uint8_t> Bytes, uint64_t Address,
                                  int &MemIdx) const {
  MemIdx = -1;
  if (Bytes.empty())
    return false;
  const unsigned B0 = Bytes[0];

  for (unsigned Op : Table[B0]) {
    const MCInstrDesc &Desc = MCII->get(Op);
    const uint64_t TS = Desc.TSFlags;
    const Form F = form(TS);
    unsigned Pos = 1;
    // Does this candidate describe these bytes?
    if (hasFixed(TS) && (Bytes.size() < 2 || Bytes[1] != fixedByte(TS)))
      continue;
    if (hasModRM(F)) {
      if (Bytes.size() < 2)
        continue;
      unsigned M = Bytes[1];
      if ((M >> 6 == 3) != rmReg(TS))
        continue;
      if (F == FormDigit && ((M >> 3) & 7) != digit(TS))
        continue;
      if (F == FormModRMSeg && ((M >> 3) & 7) > 3)
        continue;
    }

    MCInst Out;
    Out.setOpcode(Op);
    if (hasFixed(TS))
      ++Pos; // the fixed second opcode byte (AAM, AAD)
    auto Infos = Desc.operands();
    bool Ok = true;
    int Mem = -1;
    auto Imm = [&](unsigned Bytes_) -> int64_t {
      if (Pos + Bytes_ > Bytes.size()) {
        Ok = false;
        return 0;
      }
      int64_t V = Bytes_ == 1 ? int64_t(Bytes[Pos])
                              : int64_t(Bytes[Pos] | (Bytes[Pos + 1] << 8));
      if (Bytes_ == 1 && sextImm(TS))
        V = static_cast<int8_t>(Bytes[Pos]);
      Pos += Bytes_;
      return V;
    };
    const unsigned ImmBytes = imm(TS) == Imm8 ? 1 : imm(TS) == Imm16 ? 2 : 0;

    switch (F) {
    case FormNone:
    case FormRegOpc:
    case FormSegOpc: {
      const unsigned RegIdx = F == FormRegOpc ? (regOp1(TS) ? 1 : 0) : 0;
      for (unsigned K = 0, E = Infos.size(); K != E; ++K) {
        if (Infos[K].RegClass != -1) {
          unsigned N = F == FormSegOpc ? (B0 >> 3) & 3 : (K == RegIdx ? B0 & 7 : 0);
          if (F == FormRegOpc && K != RegIdx)
            N = 0;
          Out.addOperand(MCOperand::createReg(regFor(Infos[K].RegClass, N, Ok)));
        } else if (ImmBytes) {
          Out.addOperand(MCOperand::createImm(Imm(ImmBytes)));
        } else {
          Out.addOperand(MCOperand::createImm(1)); // the constant in "shl r, 1"
        }
      }
      break;
    }
    case FormModRM:
    case FormDigit:
    case FormModRMSeg: {
      Ok = modrmOperands(Out, Bytes, Pos, Desc, TS, Mem);
      if (Ok && ImmBytes) {
        Out.addOperand(MCOperand::createImm(Imm(ImmBytes)));
      } else if (Ok) {
        // Trailing fixed operands: CL in "shl r, cl", or the constant 1.
        for (unsigned K = Out.getNumOperands(), E = Infos.size(); K < E; ++K) {
          if (Infos[K].RegClass != -1)
            Out.addOperand(MCOperand::createReg(regFor(Infos[K].RegClass, 0, Ok)));
          else
            Out.addOperand(MCOperand::createImm(1));
        }
      }
      break;
    }
    case FormRel8:
    case FormRel16: {
      int64_t D = F == FormRel8 ? static_cast<int8_t>(Imm(1))
                                : static_cast<int16_t>(Imm(2));
      Out.addOperand(MCOperand::createImm((Address + Pos + D) & 0xffff));
      break;
    }
    case FormFar: {
      int64_t Off = Imm(2), Seg = Imm(2);
      Out.addOperand(MCOperand::createImm(Off));
      Out.addOperand(MCOperand::createImm(Seg));
      break;
    }
    case FormMoffs: {
      int64_t Off = Imm(2);
      SmallVector<MCOperand, 4> M = {MCOperand::createReg(MCRegister()),
                                     MCOperand::createReg(MCRegister()),
                                     MCOperand::createImm(Off),
                                     MCOperand::createReg(MCRegister())};
      bool Load = B0 < 0xA2;
      bool Dummy = true;
      MCOperand Acc = MCOperand::createReg(regFor(Infos[Load ? 0 : 4].RegClass, 0, Dummy));
      if (Load) {
        Out.addOperand(Acc);
        Mem = 1;
      } else {
        Mem = 0;
      }
      for (const MCOperand &O : M)
        Out.addOperand(O);
      if (!Load)
        Out.addOperand(Acc);
      break;
    }
    case FormJccFar:
      Ok = false;
      break;
    }
    if (!Ok)
      continue;
    MI = Out;
    Size = Pos;
    MemIdx = Mem;
    return true;
  }
  return false;
}

DecodeStatus I8086Disassembler::getInstruction(MCInst &MI, uint64_t &Size,
                                               ArrayRef<uint8_t> Bytes,
                                               uint64_t Address,
                                               raw_ostream &CS) const {
  Size = 1;
  if (Bytes.empty()) {
    Size = 0;
    return MCDisassembler::Fail;
  }
  const unsigned B0 = Bytes[0];
  const bool IsSeg = B0 == 0x26 || B0 == 0x2E || B0 == 0x36 || B0 == 0x3E;

  if (IsSeg) {
    MCInst Inner;
    uint64_t InnerSize = 0;
    int MemIdx = -1;
    if (decodeOne(Inner, InnerSize, Bytes.drop_front(1), Address + 1, MemIdx) &&
        MemIdx >= 0) {
      Inner.getOperand(MemIdx + 3).setReg(segmentRegister((B0 >> 3) & 3));
      MI = Inner;
      Size = InnerSize + 1;
      return MCDisassembler::Success;
    }
  }

  int MemIdx;
  if (decodeOne(MI, Size, Bytes, Address, MemIdx))
    return MCDisassembler::Success;
  Size = 1;
  return MCDisassembler::Fail;
}

} // end anonymous namespace

static MCDisassembler *createI8086Disassembler(const Target &T,
                                               const MCSubtargetInfo &STI,
                                               MCContext &Ctx) {
  return new I8086Disassembler(STI, Ctx,
                               std::unique_ptr<MCInstrInfo>(T.createMCInstrInfo()));
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8086Disassembler() {
  TargetRegistry::RegisterMCDisassembler(getTheI8086Target(),
                                         createI8086Disassembler);
}
