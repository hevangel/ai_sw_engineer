//===-- I8086AsmParser.cpp - Parse I8086 assembly to MCInst ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Intel syntax (spec/asm_syntax.md): "mov ax, [bx+si+4]", "add byte ptr
// es:[di], 5", "jmp short L", "jmp 0x1000:0x100", "rep movsb". Registers and
// mnemonics are case-insensitive. Memory references are always bracketed; a
// bare name is an immediate (its address), as in NASM.
//
//===----------------------------------------------------------------------===//

#include "MCTargetDesc/I8086MCTargetDesc.h"
#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/ADT/SmallVector.h"
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

#define DEBUG_TYPE "i8086-asm-parser"

using namespace llvm;

// MatchRegisterName, generated with the matcher, is used by the operand parser.
#define GET_REGISTER_MATCHER
#include "I8086GenAsmMatcher.inc"

namespace {

class I8086Operand : public MCParsedAsmOperand {
  enum KindTy { k_Token, k_Reg, k_Imm, k_Mem, k_Far } Kind;
  StringRef Tok;
  MCRegister Reg;
  const MCExpr *Imm = nullptr;
  const MCExpr *FarSeg = nullptr;
  struct {
    MCRegister Base, Index, Seg;
    const MCExpr *Disp;
    unsigned Size; // bytes: 0 unspecified, 1, 2 or 4
  } Mem{};
  SMLoc Start, End;

public:
  I8086Operand(StringRef Tok, SMLoc S)
      : Kind(k_Token), Tok(Tok), Start(S), End(S) {}
  I8086Operand(MCRegister R, SMLoc S, SMLoc E)
      : Kind(k_Reg), Reg(R), Start(S), End(E) {}
  I8086Operand(const MCExpr *Imm, SMLoc S, SMLoc E)
      : Kind(k_Imm), Imm(Imm), Start(S), End(E) {}
  I8086Operand(const MCExpr *Seg, const MCExpr *Off, SMLoc S, SMLoc E)
      : Kind(k_Far), Imm(Off), FarSeg(Seg), Start(S), End(E) {}
  I8086Operand(MCRegister Base, MCRegister Index, MCRegister Seg,
               const MCExpr *Disp, unsigned Size, SMLoc S, SMLoc E)
      : Kind(k_Mem), Start(S), End(E) {
    Mem = {Base, Index, Seg, Disp, Size};
  }

  bool isToken() const override { return Kind == k_Token; }
  bool isReg() const override { return Kind == k_Reg; }
  bool isImm() const override { return Kind == k_Imm; }
  bool isMem() const override { return Kind == k_Mem; }
  SMLoc getStartLoc() const override { return Start; }
  SMLoc getEndLoc() const override { return End; }
  StringRef getToken() const { return Tok; }
  MCRegister getReg() const override { return Reg; }

  // Immediate predicates. A relocatable expression always matches the widest
  // class that has a fixup; a constant must fit.
  bool constant(int64_t &V) const {
    return Kind == k_Imm && Imm->evaluateAsAbsolute(V);
  }
  bool isImm8() const {
    int64_t V;
    return Kind == k_Imm && (!constant(V) || (V >= -128 && V <= 255));
  }
  bool isSImm8() const {
    int64_t V;
    return constant(V) && V >= -128 && V <= 127;
  }
  bool isImm16() const {
    int64_t V;
    return Kind == k_Imm && (!constant(V) || (V >= -32768 && V <= 65535));
  }
  bool isImm16Big() const {
    int64_t V;
    return Kind == k_Imm &&
           (!constant(V) || ((V < -128 || V > 127) && V >= -32768 && V <= 65535));
  }
  bool isImmOne() const {
    int64_t V;
    return constant(V) && V == 1;
  }
  bool isImmThree() const {
    int64_t V;
    return constant(V) && V == 3;
  }
  bool isBrTarget() const { return Kind == k_Imm; }
  bool isFarPtr() const { return Kind == k_Far; }

  // Memory predicates; "x" classes need the size keyword.
  bool sized(unsigned S, bool Explicit) const {
    return Kind == k_Mem && (Mem.Size == S || (!Explicit && Mem.Size == 0));
  }
  bool noRegs() const { return !Mem.Base && !Mem.Index; }
  bool isMemAny() const { return Kind == k_Mem; }
  bool isMem8() const { return sized(1, false); }
  bool isMem16() const { return sized(2, false); }
  bool isMem32() const { return sized(4, false); }
  bool isMem8x() const { return sized(1, true); }
  bool isMem16x() const { return sized(2, true); }
  bool isMem32x() const { return sized(4, true); }
  bool isMemAbs8() const { return sized(1, false) && noRegs(); }
  bool isMemAbs16() const { return sized(2, false) && noRegs(); }

  void addRegOperands(MCInst &Inst, unsigned N) const {
    Inst.addOperand(MCOperand::createReg(Reg));
  }
  static void addExpr(MCInst &Inst, const MCExpr *E) {
    int64_t V;
    if (E->evaluateAsAbsolute(V))
      Inst.addOperand(MCOperand::createImm(V));
    else
      Inst.addOperand(MCOperand::createExpr(E));
  }
  void addImmOperands(MCInst &Inst, unsigned N) const { addExpr(Inst, Imm); }
  void addFarOperands(MCInst &Inst, unsigned N) const {
    addExpr(Inst, Imm);
    addExpr(Inst, FarSeg);
  }
  void addMemOperands(MCInst &Inst, unsigned N) const {
    Inst.addOperand(MCOperand::createReg(Mem.Base));
    Inst.addOperand(MCOperand::createReg(Mem.Index));
    addExpr(Inst, Mem.Disp);
    Inst.addOperand(MCOperand::createReg(Mem.Seg));
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
    case k_Far:
      OS << "Far";
      break;
    case k_Mem:
      OS << "Mem:size=" << Mem.Size;
      break;
    }
  }
};

class I8086AsmParser : public MCTargetAsmParser {
  MCAsmParser &Parser;
  /// Prefix instructions seen before the mnemonic of the statement being parsed.
  SmallVector<unsigned, 2> Prefixes;

  bool matchAndEmitInstruction(SMLoc IDLoc, unsigned &Opcode,
                               OperandVector &Operands, MCStreamer &Out,
                               uint64_t &ErrorInfo,
                               bool MatchingInlineAsm) override;
  bool parseRegister(MCRegister &Reg, SMLoc &StartLoc, SMLoc &EndLoc) override;
  ParseStatus tryParseRegister(MCRegister &Reg, SMLoc &StartLoc,
                               SMLoc &EndLoc) override;
  bool parseInstruction(ParseInstructionInfo &Info, StringRef Name,
                        SMLoc NameLoc, OperandVector &Operands) override;
  ParseStatus parseDirective(AsmToken DirectiveID) override;

  MCRegister regNamed(StringRef Name) const {
    return MatchRegisterName(Name.lower());
  }
  bool parseOperand(OperandVector &Operands);
  bool parseMemory(OperandVector &Operands, SMLoc S, unsigned Size,
                   MCRegister Seg);

#define GET_ASSEMBLER_HEADER
#include "I8086GenAsmMatcher.inc"

public:
  I8086AsmParser(const MCSubtargetInfo &STI, MCAsmParser &Parser,
                 const MCInstrInfo &MII)
      : MCTargetAsmParser(STI, MII), Parser(Parser) {
    MCAsmParserExtension::Initialize(Parser);
    setAvailableFeatures(ComputeAvailableFeatures(STI.getFeatureBits()));
  }
};

} // end anonymous namespace

/// .word expr[, expr...]: 16-bit data (the generic parser has .short and .value).
ParseStatus I8086AsmParser::parseDirective(AsmToken DirectiveID) {
  if (DirectiveID.getString() != ".word")
    return ParseStatus::NoMatch;
  auto Parse = [&]() -> bool {
    const MCExpr *E;
    SMLoc Loc = Parser.getTok().getLoc();
    if (Parser.parseExpression(E))
      return true;
    getStreamer().emitValue(E, 2, Loc);
    return false;
  };
  if (Parser.parseMany(Parse))
    return ParseStatus::Failure;
  return ParseStatus::Success;
}

static bool isSegment(MCRegister R) {
  return R == I8086::ES || R == I8086::CS || R == I8086::SS || R == I8086::DS;
}

bool I8086AsmParser::parseRegister(MCRegister &Reg, SMLoc &StartLoc,
                                   SMLoc &EndLoc) {
  return tryParseRegister(Reg, StartLoc, EndLoc).isNoMatch();
}

ParseStatus I8086AsmParser::tryParseRegister(MCRegister &Reg, SMLoc &StartLoc,
                                             SMLoc &EndLoc) {
  const AsmToken &Tok = Parser.getTok();
  StartLoc = Tok.getLoc();
  EndLoc = Tok.getEndLoc();
  if (Tok.isNot(AsmToken::Identifier))
    return ParseStatus::NoMatch;
  Reg = regNamed(Tok.getString());
  if (!Reg)
    return ParseStatus::NoMatch;
  Parser.Lex();
  return ParseStatus::Success;
}

/// Parse "[term (+|-) term ...]": registers BX/BP (base), SI/DI (index) and
/// constant or symbolic displacements. The opening bracket is the current token.
bool I8086AsmParser::parseMemory(OperandVector &Operands, SMLoc S,
                                 unsigned Size, MCRegister Seg) {
  AsmLexer &Lexer = Parser.getLexer();
  Parser.Lex(); // '['
  MCRegister Base, Index;
  const MCExpr *Disp = nullptr;
  bool First = true;
  while (true) {
    bool Negative = false;
    if (Lexer.is(AsmToken::Plus)) {
      Parser.Lex();
    } else if (Lexer.is(AsmToken::Minus)) {
      Negative = true;
      Parser.Lex();
    } else if (!First) {
      return Error(Lexer.getLoc(), "expected '+', '-' or ']' in memory operand");
    }
    First = false;
    SMLoc TermLoc = Lexer.getLoc();
    MCRegister R;
    if (Lexer.is(AsmToken::Identifier))
      R = regNamed(Parser.getTok().getString());
    if (R) {
      if (Negative)
        return Error(TermLoc, "a register cannot be subtracted");
      Parser.Lex();
      if (R == I8086::BX || R == I8086::BP) {
        if (Base)
          return Error(TermLoc, "two base registers in memory operand");
        Base = R;
      } else if (R == I8086::SI || R == I8086::DI) {
        if (Index)
          return Error(TermLoc, "two index registers in memory operand");
        Index = R;
      } else {
        return Error(TermLoc, "only bx, bp, si and di can address memory");
      }
    } else {
      const MCExpr *Term;
      SMLoc EndLoc;
      if (Parser.parsePrimaryExpr(Term, EndLoc, nullptr))
        return true;
      if (Negative)
        Term = MCUnaryExpr::createMinus(Term, getContext());
      Disp = Disp ? MCBinaryExpr::createAdd(Disp, Term, getContext()) : Term;
    }
    if (Lexer.is(AsmToken::RBrac))
      break;
  }
  SMLoc E = Lexer.getTok().getEndLoc();
  Parser.Lex(); // ']'
  if (!Disp)
    Disp = MCConstantExpr::create(0, getContext());
  if (!Base && !Index) {
    // Only a displacement: a direct address.
  }
  Operands.push_back(std::make_unique<I8086Operand>(Base, Index, Seg, Disp,
                                                    Size, S, E));
  return false;
}

bool I8086AsmParser::parseOperand(OperandVector &Operands) {
  SMLoc S = Parser.getTok().getLoc();
  AsmLexer &Lexer = Parser.getLexer();

  unsigned Size = 0;
  MCRegister Seg;
  bool SawQualifier = false;

  if (Lexer.is(AsmToken::Identifier)) {
    std::string Word = Parser.getTok().getString().lower();
    // Branch distance keywords are tokens of the instruction's syntax.
    if (Word == "short" || Word == "near" || Word == "far") {
      Operands.push_back(std::make_unique<I8086Operand>(
          StringRef(getContext().allocateString(Word)), S));
      Parser.Lex();
      return parseOperand(Operands);
    }
    unsigned Bytes = StringSwitch<unsigned>(Word)
                         .Case("byte", 1)
                         .Case("word", 2)
                         .Case("dword", 4)
                         .Default(0);
    if (Bytes) {
      Size = Bytes;
      SawQualifier = true;
      Parser.Lex();
      if (Lexer.is(AsmToken::Identifier) &&
          Parser.getTok().getString().equals_insensitive("ptr"))
        Parser.Lex();
    }
  }

  // Optional segment override: "es:[bx]".
  if (Lexer.is(AsmToken::Identifier)) {
    MCRegister R = regNamed(Parser.getTok().getString());
    if (R && isSegment(R)) {
      AsmToken Next = Lexer.peekTok();
      if (Next.is(AsmToken::Colon)) {
        Seg = R;
        Parser.Lex(); // register
        Parser.Lex(); // ':'
        SawQualifier = true;
        if (Lexer.isNot(AsmToken::LBrac))
          return Error(Lexer.getLoc(), "expected '[' after a segment override");
      }
    }
  }

  if (Lexer.is(AsmToken::LBrac))
    return parseMemory(Operands, S, Size, Seg);
  if (SawQualifier)
    return Error(Lexer.getLoc(), "expected a memory operand after the size or segment");

  if (Lexer.is(AsmToken::Identifier)) {
    MCRegister R = regNamed(Parser.getTok().getString());
    if (R) {
      SMLoc E = Parser.getTok().getEndLoc();
      Parser.Lex();
      Operands.push_back(std::make_unique<I8086Operand>(R, S, E));
      return false;
    }
  }

  const MCExpr *Expr;
  SMLoc E;
  if (Parser.parseExpression(Expr, E))
    return true;
  if (Lexer.is(AsmToken::Colon)) {
    // segment:offset, as in "jmp 0x1000:0x100".
    Parser.Lex();
    const MCExpr *Off;
    if (Parser.parseExpression(Off, E))
      return true;
    Operands.push_back(std::make_unique<I8086Operand>(Expr, Off, S, E));
    return false;
  }
  Operands.push_back(std::make_unique<I8086Operand>(Expr, S, E));
  return false;
}

static unsigned prefixOpcode(StringRef Name) {
  return StringSwitch<unsigned>(Name)
      .Cases({"rep", "repe", "repz"}, I8086::REP)
      .Cases({"repne", "repnz"}, I8086::REPNE)
      .Case("lock", I8086::LOCK)
      .Case("es", I8086::SEG_ES)
      .Case("cs", I8086::SEG_CS)
      .Case("ss", I8086::SEG_SS)
      .Case("ds", I8086::SEG_DS)
      .Default(0);
}

bool I8086AsmParser::parseInstruction(ParseInstructionInfo &Info,
                                      StringRef Name, SMLoc NameLoc,
                                      OperandVector &Operands) {
  Prefixes.clear();
  std::string Lower = Name.lower();
  // "rep movsb": the prefix is its own instruction, emitted before the next.
  while (unsigned P = prefixOpcode(Lower)) {
    if (getLexer().isNot(AsmToken::Identifier))
      break;
    Prefixes.push_back(P);
    NameLoc = getLexer().getLoc();
    Lower = Parser.getTok().getString().lower();
    Parser.Lex();
  }
  Operands.push_back(std::make_unique<I8086Operand>(
      StringRef(getContext().allocateString(Lower)), NameLoc));
  if (getLexer().is(AsmToken::EndOfStatement)) {
    Parser.Lex();
    return false;
  }
  while (true) {
    if (parseOperand(Operands))
      return true;
    if (getLexer().isNot(AsmToken::Comma))
      break;
    Parser.Lex();
  }
  if (getLexer().isNot(AsmToken::EndOfStatement)) {
    SMLoc Loc = getLexer().getLoc();
    Parser.eatToEndOfStatement();
    return Error(Loc, "unexpected token after operand");
  }
  Parser.Lex();
  return false;
}

bool I8086AsmParser::matchAndEmitInstruction(SMLoc IDLoc, unsigned &Opcode,
                                             OperandVector &Operands,
                                             MCStreamer &Out,
                                             uint64_t &ErrorInfo,
                                             bool MatchingInlineAsm) {
  MCInst Inst;
  switch (MatchInstructionImpl(Operands, Inst, ErrorInfo, MatchingInlineAsm)) {
  case Match_Success:
    for (unsigned P : Prefixes) {
      MCInst Pre;
      Pre.setOpcode(P);
      Pre.setLoc(IDLoc);
      Out.emitInstruction(Pre, getSTI());
    }
    Prefixes.clear();
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
      Loc = ((I8086Operand &)*Operands[ErrorInfo]).getStartLoc();
    }
    return Error(Loc, "invalid operand for instruction");
  }
  default:
    return Error(IDLoc, "invalid instruction");
  }
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8086AsmParser() {
  RegisterMCAsmParser<I8086AsmParser> X(getTheI8086Target());
}

#define GET_MATCHER_IMPLEMENTATION
#include "I8086GenAsmMatcher.inc"
