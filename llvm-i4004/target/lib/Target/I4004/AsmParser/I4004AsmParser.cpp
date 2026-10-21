//===-- I4004AsmParser.cpp - Parse I4004 assembly to MCInst --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Parses Intel 4004 (MCS-4) assembly (spec/asm_syntax.md) into MCInsts.
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I4004MCTargetDesc.h"
#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/ADT/StringSwitch.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCExpr.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCParser/AsmLexer.h"
#include "llvm/MC/MCParser/MCParsedAsmOperand.h"
#include "llvm/MC/MCParser/MCTargetAsmParser.h"
#include "llvm/MC/MCStreamer.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Casting.h"
#include "llvm/Support/Compiler.h"

#define DEBUG_TYPE "i4004-asm-parser"

using namespace llvm;

namespace {

/// A parsed I4004 assembly operand.
class I4004Operand : public MCParsedAsmOperand {
  enum KindTy { k_Token, k_Reg, k_Imm } Kind;

  StringRef Tok;
  MCRegister Reg;
  const MCExpr *Imm = nullptr;

  SMLoc Start, End;

public:
  I4004Operand(StringRef Tok, SMLoc S)
      : Kind(k_Token), Tok(Tok), Start(S), End(S) {}
  I4004Operand(MCRegister Reg, SMLoc S, SMLoc E)
      : Kind(k_Reg), Reg(Reg), Start(S), End(E) {}
  I4004Operand(const MCExpr *Imm, SMLoc S, SMLoc E)
      : Kind(k_Imm), Imm(Imm), Start(S), End(E) {}

  bool isToken() const override { return Kind == k_Token; }
  bool isReg() const override { return Kind == k_Reg; }
  bool isImm() const override { return Kind == k_Imm; }
  bool isMem() const override { return false; }

  SMLoc getStartLoc() const override { return Start; }
  SMLoc getEndLoc() const override { return End; }

  StringRef getToken() const {
    assert(Kind == k_Token);
    return Tok;
  }
  MCRegister getReg() const override {
    assert(Kind == k_Reg);
    return Reg;
  }
  const MCExpr *getImm() const {
    assert(Kind == k_Imm);
    return Imm;
  }

  // --- operand-class predicates referenced from the .td ----------------

  bool evalImm(int64_t &Val) const {
    if (Kind != k_Imm)
      return false;
    return Imm->evaluateAsAbsolute(Val);
  }
  bool isImmExprOrInRange(int64_t Lo, int64_t Hi) const {
    if (Kind != k_Imm)
      return false;
    int64_t Val;
    if (!Imm->evaluateAsAbsolute(Val))
      return true; // symbol / expr: defer to a fixup
    return Val >= Lo && Val <= Hi;
  }

  bool isImm4() const { return isImmExprOrInRange(0, 15); }
  bool isImm8() const { return isImmExprOrInRange(0, 255); }
  bool isCond() const { return isImmExprOrInRange(0, 15); }
  bool isAddr8() const { return Kind == k_Imm; }
  bool isAddr12() const { return Kind == k_Imm; }

  // --- operand-rendering methods referenced from the .td ---------------

  void addExpr(MCInst &Inst, const MCExpr *Expr) const {
    if (!Expr)
      Inst.addOperand(MCOperand::createImm(0));
    else if (const auto *CE = dyn_cast<MCConstantExpr>(Expr))
      Inst.addOperand(MCOperand::createImm(CE->getValue()));
    else
      Inst.addOperand(MCOperand::createExpr(Expr));
  }

  void addRegOperands(MCInst &Inst, unsigned N) const {
    assert(N == 1 && "Invalid number of operands!");
    Inst.addOperand(MCOperand::createReg(Reg));
  }
  void addImmOperands(MCInst &Inst, unsigned N) const {
    assert(N == 1 && "Invalid number of operands!");
    addExpr(Inst, Imm);
  }

  void print(raw_ostream &OS, const MCAsmInfo &MAI) const override {
    switch (Kind) {
    case k_Token:
      OS << "Token:" << Tok;
      break;
    case k_Reg:
      OS << "Reg:" << Reg.id();
      break;
    case k_Imm:
      OS << "Imm:";
      MAI.printExpr(OS, *Imm);
      break;
    }
  }

  static std::unique_ptr<I4004Operand> createToken(StringRef Str, SMLoc S) {
    return std::make_unique<I4004Operand>(Str, S);
  }
  static std::unique_ptr<I4004Operand> createReg(MCRegister Reg, SMLoc S,
                                                 SMLoc E) {
    return std::make_unique<I4004Operand>(Reg, S, E);
  }
  static std::unique_ptr<I4004Operand> createImm(const MCExpr *Val, SMLoc S,
                                                 SMLoc E) {
    return std::make_unique<I4004Operand>(Val, S, E);
  }
};

/// Parses I4004 assembly from a stream.
class I4004AsmParser : public MCTargetAsmParser {
  MCAsmParser &Parser;

  bool matchAndEmitInstruction(SMLoc IDLoc, unsigned &Opcode,
                               OperandVector &Operands, MCStreamer &Out,
                               uint64_t &ErrorInfo,
                               bool MatchingInlineAsm) override;

  bool parseRegister(MCRegister &Reg, SMLoc &StartLoc, SMLoc &EndLoc) override;
  ParseStatus tryParseRegister(MCRegister &Reg, SMLoc &StartLoc,
                               SMLoc &EndLoc) override;

  bool parseInstruction(ParseInstructionInfo &Info, StringRef Name,
                        SMLoc NameLoc, OperandVector &Operands) override;

  ParseStatus parseDirective(AsmToken DirectiveID) override {
    return ParseStatus::NoMatch;
  }

  bool parseOperand(OperandVector &Operands);
  ParseStatus parseCond(OperandVector &Operands);

  MCRegister matchRegisterName(StringRef Name);

  MCAsmParser &getParser() const { return Parser; }
  AsmLexer &getLexer() const { return Parser.getLexer(); }

#define GET_ASSEMBLER_HEADER
#include "I4004GenAsmMatcher.inc"

public:
  I4004AsmParser(const MCSubtargetInfo &STI, MCAsmParser &Parser,
                 const MCInstrInfo &MII)
      : MCTargetAsmParser(STI, MII), Parser(Parser) {
    MCAsmParserExtension::Initialize(Parser);
    setAvailableFeatures(ComputeAvailableFeatures(STI.getFeatureBits()));
  }
};

} // end anonymous namespace

/// Map a register or pair name (case-insensitive) to its MCRegister.
MCRegister I4004AsmParser::matchRegisterName(StringRef Name) {
  std::string Lower = Name.lower();
  StringRef N(Lower);

  // Pairs: p0..p7 and the aliases r0r1 .. r14r15.
  if (N.size() >= 2 && N[0] == 'p') {
    unsigned Idx;
    if (!N.substr(1).getAsInteger(10, Idx) && Idx <= 7)
      return MCRegister(I4004::P0 + Idx);
    return MCRegister();
  }
  // Index registers: r0..r15 (not an rNrM pair).
  if (N.size() >= 2 && N[0] == 'r') {
    // rNrM alias?
    size_t SecondR = N.find('r', 1);
    if (SecondR != StringRef::npos) {
      unsigned Hi, Lo;
      if (!N.substr(1, SecondR - 1).getAsInteger(10, Hi) &&
          !N.substr(SecondR + 1).getAsInteger(10, Lo) && Hi + 1 == Lo &&
          (Hi % 2) == 0 && Lo <= 15)
        return MCRegister(I4004::P0 + Hi / 2);
      return MCRegister();
    }
    unsigned Idx;
    if (!N.substr(1).getAsInteger(10, Idx) && Idx <= 15)
      return MCRegister(I4004::R0 + Idx);
  }
  if (N == "acc")
    return MCRegister(I4004::ACC);
  if (N == "cy")
    return MCRegister(I4004::CY);
  return MCRegister();
}

bool I4004AsmParser::parseRegister(MCRegister &Reg, SMLoc &StartLoc,
                                   SMLoc &EndLoc) {
  ParseStatus Res = tryParseRegister(Reg, StartLoc, EndLoc);
  if (Res.isSuccess())
    return false;
  return Error(StartLoc, "invalid register name");
}

ParseStatus I4004AsmParser::tryParseRegister(MCRegister &Reg, SMLoc &StartLoc,
                                             SMLoc &EndLoc) {
  if (getLexer().isNot(AsmToken::Identifier))
    return ParseStatus::NoMatch;
  StartLoc = getParser().getTok().getLoc();
  EndLoc = getParser().getTok().getEndLoc();
  Reg = matchRegisterName(getParser().getTok().getString());
  if (!Reg)
    return ParseStatus::NoMatch;
  getParser().Lex();
  return ParseStatus::Success;
}

/// Parse a JCN condition: a hex/decimal nibble, or a mnemonic combo of
/// Z (ACC=0,C2=4), C (CY=1,C3=2), T (TEST=0,C4=1) with a leading N for the
/// invert bit (C1=8). asm_syntax.md.
ParseStatus I4004AsmParser::parseCond(OperandVector &Operands) {
  SMLoc S = getParser().getTok().getLoc();

  if (getLexer().is(AsmToken::Identifier)) {
    StringRef Id = getParser().getTok().getString();
    std::string Lower = Id.lower();
    StringRef N(Lower);
    unsigned Val = 0;
    bool Ok = !N.empty();
    size_t i = 0;
    if (Ok && N[0] == 'n') { // invert
      Val |= 0x8;
      i = 1;
      if (N.size() == 1)
        Ok = false; // bare "n" is not a condition
    }
    for (; i < N.size() && Ok; ++i) {
      switch (N[i]) {
      case 'z': Val |= 0x4; break; // ACC = 0  (C2)
      case 'c': Val |= 0x2; break; // CY = 1   (C3)
      case 't': Val |= 0x1; break; // TEST = 0 (C4)
      default:  Ok = false; break;
      }
    }
    if (Ok) {
      getParser().Lex();
      SMLoc E = getParser().getTok().getLoc();
      Operands.push_back(I4004Operand::createImm(
          MCConstantExpr::create(Val, getContext()), S, E));
      return ParseStatus::Success;
    }
    // Not a condition mnemonic: fall through to expression parsing.
  }

  // Numeric / expression condition.
  const MCExpr *Expr;
  SMLoc E;
  if (getParser().parseExpression(Expr, E))
    return ParseStatus::Failure;
  Operands.push_back(I4004Operand::createImm(Expr, S, E));
  return ParseStatus::Success;
}

bool I4004AsmParser::parseOperand(OperandVector &Operands) {
  // Register?
  MCRegister Reg;
  SMLoc S, E;
  ParseStatus Res = tryParseRegister(Reg, S, E);
  if (Res.isSuccess()) {
    Operands.push_back(I4004Operand::createReg(Reg, S, E));
    return false;
  }
  if (Res.isFailure())
    return true;

  // Immediate / expression / symbol -- possibly a %lo8(...) / %page(...)
  // code-label specifier (abi.md sec.4.4.1). The AsmLexer tokenizes '%' as
  // AsmToken::Percent; match it then the specifier name and a parenthesised
  // subexpression.
  S = getParser().getTok().getLoc();
  if (getLexer().is(AsmToken::Percent)) {
    getParser().Lex(); // eat '%'
    if (getLexer().isNot(AsmToken::Identifier))
      return true;
    StringRef Spec = getParser().getTok().getString();
    uint16_t Kind;
    if (Spec == "lo8")
      Kind = I4004::S_LO8;
    else if (Spec == "page")
      Kind = I4004::S_PAGE;
    else
      return Error(getParser().getTok().getLoc(),
                   "unknown operand specifier (expected %lo8 or %page)");
    getParser().Lex(); // eat the specifier name
    if (getLexer().isNot(AsmToken::LParen))
      return Error(getParser().getTok().getLoc(), "expected '(' after specifier");
    getParser().Lex(); // eat '('
    const MCExpr *Sub;
    if (getParser().parseExpression(Sub, E))
      return true;
    if (getLexer().isNot(AsmToken::RParen))
      return Error(getParser().getTok().getLoc(), "expected ')' to close specifier");
    getParser().Lex(); // eat ')'
    const MCExpr *SE = MCSpecifierExpr::create(Sub, Kind, getContext(), S);
    Operands.push_back(I4004Operand::createImm(SE, S, E));
    return false;
  }
  const MCExpr *Expr;
  if (getParser().parseExpression(Expr, E))
    return true;
  Operands.push_back(I4004Operand::createImm(Expr, S, E));
  return false;
}

bool I4004AsmParser::parseInstruction(ParseInstructionInfo &Info,
                                      StringRef Name, SMLoc NameLoc,
                                      OperandVector &Operands) {
  Operands.push_back(I4004Operand::createToken(Name.lower(), NameLoc));

  // No operands?
  if (getLexer().is(AsmToken::EndOfStatement)) {
    getParser().Lex();
    return false;
  }

  // JCN's first operand is a condition code (mnemonic combo or nibble); route
  // it through the dedicated parser so Z/C/T/N... map to the right OPA bits.
  bool First = true;
  std::string Mnem = Name.lower();
  if (Mnem == "jcn" && First) {
    if (parseCond(Operands).isFailure())
      return true;
    First = false;
    // Expect a comma then the address operand.
    if (getLexer().is(AsmToken::Comma)) {
      getParser().Lex();
      if (parseOperand(Operands))
        return true;
    }
  } else {
    if (parseOperand(Operands))
      return true;
    First = false;
  }

  while (getLexer().is(AsmToken::Comma)) {
    getParser().Lex(); // eat ','
    if (parseOperand(Operands))
      return true;
  }

  if (getLexer().isNot(AsmToken::EndOfStatement)) {
    SMLoc Loc = getLexer().getLoc();
    getParser().eatToEndOfStatement();
    return Error(Loc, "unexpected token in operand list");
  }
  getParser().Lex(); // consume EndOfStatement
  return false;
}

bool I4004AsmParser::matchAndEmitInstruction(SMLoc IDLoc, unsigned &Opcode,
                                             OperandVector &Operands,
                                             MCStreamer &Out,
                                             uint64_t &ErrorInfo,
                                             bool MatchingInlineAsm) {
  MCInst Inst;
  unsigned MatchResult =
      MatchInstructionImpl(Operands, Inst, ErrorInfo, MatchingInlineAsm);

  switch (MatchResult) {
  case Match_Success:
    Inst.setLoc(IDLoc);
    Out.emitInstruction(Inst, getSTI());
    return false;
  case Match_MnemonicFail:
    return Error(IDLoc, "invalid instruction mnemonic");
  case Match_InvalidOperand: {
    SMLoc ErrorLoc = IDLoc;
    if (ErrorInfo != ~0ULL) {
      if (ErrorInfo >= Operands.size())
        return Error(IDLoc, "too few operands for instruction");
      ErrorLoc = ((I4004Operand &)*Operands[ErrorInfo]).getStartLoc();
      if (ErrorLoc == SMLoc())
        ErrorLoc = IDLoc;
    }
    return Error(ErrorLoc, "invalid operand for instruction");
  }
  }
  llvm_unreachable("Unknown match type detected!");
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI4004AsmParser() {
  RegisterMCAsmParser<I4004AsmParser> X(getTheI4004Target());
}

#define GET_REGISTER_MATCHER
#define GET_MATCHER_IMPLEMENTATION
#include "I4004GenAsmMatcher.inc"
