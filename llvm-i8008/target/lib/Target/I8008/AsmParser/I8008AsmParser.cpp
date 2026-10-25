//===-- I8008AsmParser.cpp - Parse I8008 assembly to MCInst --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Parses spec/asm_syntax.md: the register names are part of the mnemonic, so
// an instruction has at most one operand, an immediate / address expression,
// optionally wrapped in %lo(...) or %hi(...).
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8008MCTargetDesc.h"
#include "TargetInfo/I8008TargetInfo.h"
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

#define DEBUG_TYPE "i8008-asm-parser"

using namespace llvm;

namespace {

class I8008Operand : public MCParsedAsmOperand {
  enum KindTy { k_Token, k_Imm } Kind;
  StringRef Tok;
  const MCExpr *Imm = nullptr;
  SMLoc Start, End;

public:
  I8008Operand(StringRef Tok, SMLoc S)
      : Kind(k_Token), Tok(Tok), Start(S), End(S) {}
  I8008Operand(const MCExpr *Imm, SMLoc S, SMLoc E)
      : Kind(k_Imm), Imm(Imm), Start(S), End(E) {}

  bool isToken() const override { return Kind == k_Token; }
  bool isReg() const override { return false; }
  bool isImm() const override { return Kind == k_Imm; }
  bool isMem() const override { return false; }
  SMLoc getStartLoc() const override { return Start; }
  SMLoc getEndLoc() const override { return End; }
  StringRef getToken() const { return Tok; }
  MCRegister getReg() const override { llvm_unreachable("no register operands"); }

  // Constant in [Lo, Hi], or an expression left to a fixup.
  bool inRangeOrExpr(int64_t Lo, int64_t Hi) const {
    if (Kind != k_Imm)
      return false;
    int64_t V;
    if (!Imm->evaluateAsAbsolute(V))
      return true;
    return V >= Lo && V <= Hi;
  }
  bool isImm8() const { return inRangeOrExpr(-128, 255); }
  bool isAddr16() const { return inRangeOrExpr(0, 0x3fff); }
  template <int Lo, int Hi> bool isConstIn() const {
    int64_t V;
    return Kind == k_Imm && Imm->evaluateAsAbsolute(V) && V >= Lo && V <= Hi;
  }

  void addRegOperands(MCInst &Inst, unsigned N) const {
    llvm_unreachable("the 8008 has no register operands");
  }
  void addImmOperands(MCInst &Inst, unsigned N) const {
    if (const auto *CE = dyn_cast<MCConstantExpr>(Imm))
      Inst.addOperand(MCOperand::createImm(CE->getValue()));
    else
      Inst.addOperand(MCOperand::createExpr(Imm));
  }

  void print(raw_ostream &OS, const MCAsmInfo &MAI) const override {
    if (Kind == k_Token) {
      OS << "Token:" << Tok;
    } else {
      OS << "Imm:";
      MAI.printExpr(OS, *Imm);
    }
  }
};

class I8008AsmParser : public MCTargetAsmParser {
  MCAsmParser &Parser;

  bool matchAndEmitInstruction(SMLoc IDLoc, unsigned &Opcode,
                               OperandVector &Operands, MCStreamer &Out,
                               uint64_t &ErrorInfo,
                               bool MatchingInlineAsm) override;
  bool parseRegister(MCRegister &Reg, SMLoc &StartLoc, SMLoc &EndLoc) override {
    return Error(StartLoc, "the 8008 has no register operands");
  }
  ParseStatus tryParseRegister(MCRegister &Reg, SMLoc &StartLoc,
                               SMLoc &EndLoc) override {
    return ParseStatus::NoMatch;
  }
  bool parseInstruction(ParseInstructionInfo &Info, StringRef Name,
                        SMLoc NameLoc, OperandVector &Operands) override;
  ParseStatus parseDirective(AsmToken DirectiveID) override {
    return ParseStatus::NoMatch;
  }
  bool parseOperand(OperandVector &Operands);

#define GET_ASSEMBLER_HEADER
#include "I8008GenAsmMatcher.inc"

public:
  I8008AsmParser(const MCSubtargetInfo &STI, MCAsmParser &Parser,
                 const MCInstrInfo &MII)
      : MCTargetAsmParser(STI, MII), Parser(Parser) {
    MCAsmParserExtension::Initialize(Parser);
    setAvailableFeatures(ComputeAvailableFeatures(STI.getFeatureBits()));
  }
};

} // end anonymous namespace

bool I8008AsmParser::parseOperand(OperandVector &Operands) {
  SMLoc S = Parser.getTok().getLoc(), E;
  AsmLexer &Lexer = Parser.getLexer();
  if (Lexer.is(AsmToken::Percent)) {
    Parser.Lex();
    if (Lexer.isNot(AsmToken::Identifier))
      return Error(Parser.getTok().getLoc(), "expected %lo or %hi");
    StringRef Spec = Parser.getTok().getString();
    uint16_t Kind;
    if (Spec.equals_insensitive("lo"))
      Kind = I8008::S_LO;
    else if (Spec.equals_insensitive("hi"))
      Kind = I8008::S_HI;
    else
      return Error(Parser.getTok().getLoc(), "expected %lo or %hi");
    Parser.Lex();
    if (Lexer.isNot(AsmToken::LParen))
      return Error(Parser.getTok().getLoc(), "expected '('");
    Parser.Lex();
    const MCExpr *Sub;
    if (Parser.parseExpression(Sub, E))
      return true;
    if (Lexer.isNot(AsmToken::RParen))
      return Error(Parser.getTok().getLoc(), "expected ')'");
    E = Parser.getTok().getEndLoc();
    Parser.Lex();
    // Fold a constant right away; keep a relocatable one for the fixup.
    int64_t V;
    const MCExpr *Res;
    if (Sub->evaluateAsAbsolute(V))
      Res = MCConstantExpr::create(Kind == I8008::S_LO ? (V & 0xff)
                                                       : ((V >> 8) & 0xff),
                                   getContext());
    else
      Res = MCSpecifierExpr::create(Sub, Kind, getContext(), S);
    Operands.push_back(std::make_unique<I8008Operand>(Res, S, E));
    return false;
  }
  const MCExpr *Expr;
  if (Parser.parseExpression(Expr, E))
    return true;
  Operands.push_back(std::make_unique<I8008Operand>(Expr, S, E));
  return false;
}

bool I8008AsmParser::parseInstruction(ParseInstructionInfo &Info,
                                      StringRef Name, SMLoc NameLoc,
                                      OperandVector &Operands) {
  Operands.push_back(std::make_unique<I8008Operand>(
      StringRef(getContext().allocateString(Name.lower())), NameLoc));
  if (getLexer().is(AsmToken::EndOfStatement)) {
    Parser.Lex();
    return false;
  }
  if (parseOperand(Operands))
    return true;
  if (getLexer().isNot(AsmToken::EndOfStatement)) {
    SMLoc Loc = getLexer().getLoc();
    Parser.eatToEndOfStatement();
    return Error(Loc, "unexpected token after operand");
  }
  Parser.Lex();
  return false;
}

bool I8008AsmParser::matchAndEmitInstruction(SMLoc IDLoc, unsigned &Opcode,
                                             OperandVector &Operands,
                                             MCStreamer &Out,
                                             uint64_t &ErrorInfo,
                                             bool MatchingInlineAsm) {
  MCInst Inst;
  switch (MatchInstructionImpl(Operands, Inst, ErrorInfo, MatchingInlineAsm)) {
  case Match_Success:
    Inst.setLoc(IDLoc);
    Out.emitInstruction(Inst, getSTI());
    return false;
  case Match_MnemonicFail:
    return Error(IDLoc, "invalid instruction mnemonic");
  case Match_InvalidOperand: {
    SMLoc Loc = IDLoc;
    if (ErrorInfo != ~0ULL) {
      if (ErrorInfo >= Operands.size())
        return Error(IDLoc, "too few operands for instruction");
      Loc = ((I8008Operand &)*Operands[ErrorInfo]).getStartLoc();
    }
    return Error(Loc, "invalid operand for instruction");
  }
  default:
    return Error(IDLoc, "invalid instruction");
  }
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8008AsmParser() {
  RegisterMCAsmParser<I8008AsmParser> X(getTheI8008Target());
}

#define GET_MATCHER_IMPLEMENTATION
#include "I8008GenAsmMatcher.inc"
