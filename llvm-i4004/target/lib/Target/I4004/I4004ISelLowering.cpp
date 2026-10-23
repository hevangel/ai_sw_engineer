//===-- I4004ISelLowering.cpp - I4004 DAG Lowering Implementation --------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004ISelLowering.h"
#include "I4004.h"
#include "I4004RegisterInfo.h"
#include "I4004Subtarget.h"
#include "I4004TargetMachine.h"
#include "llvm/CodeGen/CallingConvLower.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include "llvm/CodeGen/SelectionDAG.h"
#include "llvm/CodeGen/TargetLoweringObjectFileImpl.h"
#include "llvm/IR/GlobalValue.h"
#include "llvm/IR/RuntimeLibcalls.h"
#include "llvm/Support/Debug.h"
#include "llvm/Support/KnownBits.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-lower"

#define GET_SDNODE_ENUM
#include "I4004GenSDNodeInfo.inc"

#define GET_CALLING_CONV_IMPL
#include "I4004GenCallingConv.inc"

I4004TargetLowering::I4004TargetLowering(const TargetMachine &TM,
                                         const I4004Subtarget &STI)
    : TargetLowering(TM, STI), Subtarget(STI) {
  // Legal integer types: i8 in a register pair (PR8), i16 in a double-pair
  // (DR16). This follows AVR (i8 in GPR8, i16 in DREGS): i16 is legal so
  // pointers stay a single value, but i16 ARITHMETIC is expanded to i8 ops on
  // the two sub-pairs (abi.md sec.1.2: i16 is not a native width).
  addRegisterClass(MVT::i8, &I4004::PR8RegClass);
  addRegisterClass(MVT::i16, &I4004::DR16RegClass);

  computeRegisterProperties(STI.getRegisterInfo());

  setStackPointerRegisterToSaveRestore(I4004::P7);
  setBooleanContents(ZeroOrOneBooleanContent);

  setOperationAction(ISD::Constant, MVT::i8, Legal);

  // Global/block addresses are 16-bit handles wrapped by our WRAPPER node.
  setOperationAction(ISD::GlobalAddress, MVT::i16, Custom);
  setOperationAction(ISD::BlockAddress, MVT::i16, Custom);

  // Loads and stores: selected by TableGen patterns over a DR16 pointer
  // (LDRAMi8/STRAMi8 pseudos -> SRC+RDM/WRM). Only i8 access is legal; wider
  // and extending/truncating forms reduce to i8.
  for (MVT VT : MVT::integer_valuetypes()) {
    setLoadExtAction(ISD::EXTLOAD, VT, MVT::i8, Expand);
    setLoadExtAction(ISD::ZEXTLOAD, VT, MVT::i8, Expand);
    setLoadExtAction(ISD::SEXTLOAD, VT, MVT::i8, Expand);
  }
  setTruncStoreAction(MVT::i16, MVT::i8, Expand);

  // Sign-extend is expanded (the BCD firmware is unsigned; zext/anyext are
  // handled by pseudos). Refine if a signed consumer appears.
  setOperationAction(ISD::SIGN_EXTEND, MVT::i16, Expand);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i16, Expand);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i8, Expand);

  // i8 ALU the 4004 lacks natively -> libcalls (libi4004rt, abi.md sec.6) or
  // expand. AND/OR/XOR are expanded via nibble sequences in the post-RA pass
  // (no native bitwise ALU); here they are Expand so DAG tries the generic
  // route, with i8 MUL/DIV/shift as libcalls.
  setOperationAction(ISD::MUL, MVT::i8, LibCall);
  setOperationAction(ISD::SDIV, MVT::i8, LibCall);
  setOperationAction(ISD::UDIV, MVT::i8, LibCall);
  setOperationAction(ISD::SREM, MVT::i8, LibCall);
  setOperationAction(ISD::UREM, MVT::i8, LibCall);
  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::ROTL, VT, Expand);
    setOperationAction(ISD::ROTR, VT, Expand);
  }

  // i16 arithmetic: MUL/DIV/REM -> libcalls; shifts are custom-lowered below.
  setOperationAction(ISD::MUL, MVT::i16, LibCall);
  setOperationAction(ISD::SDIV, MVT::i16, LibCall);
  setOperationAction(ISD::UDIV, MVT::i16, LibCall);
  setOperationAction(ISD::SREM, MVT::i16, LibCall);
  setOperationAction(ISD::UREM, MVT::i16, LibCall);
  // i32 arithmetic (rare; the firmware's digit scaling uses i32 udiv/mul by 10)
  // -> libcalls. i32 is split by the legalizer where it can; mul/div go to the
  // si3 helpers.
  setOperationAction(ISD::MUL, MVT::i32, LibCall);
  setOperationAction(ISD::SDIV, MVT::i32, LibCall);
  setOperationAction(ISD::UDIV, MVT::i32, LibCall);
  setOperationAction(ISD::SREM, MVT::i32, LibCall);
  setOperationAction(ISD::UREM, MVT::i32, LibCall);

  // AND/OR/XOR: the 4004 has no native bitwise ALU. Custom-lower to libi4004rt
  // helpers (__and/__or/__xor qi3/hi3). The runtime agent provides these; names
  // follow abi.md sec.6's helper convention.
  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::AND, VT, Custom);
    setOperationAction(ISD::OR, VT, Custom);
    setOperationAction(ISD::XOR, VT, Custom);
  }

  setMinFunctionAlignment(Align(1));
  setPrefFunctionAlignment(Align(1));

  // mul/div/rem libcall impl names are registered in
  // I4004Subtarget::initLibcallLoweringInfo (the LLVM 23 hook).

  // Variable shifts have no native 4004 instruction and no standard i8 RTLIB
  // entry, so custom-lower them (like AND/OR/XOR) to explicit libi4004rt calls
  // __{ashl,lshr,ashr}{qi,hi}3 (abi.md sec.6). PARTS forms expand first.
  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::SHL, VT, Custom);
    setOperationAction(ISD::SRL, VT, Custom);
    setOperationAction(ISD::SRA, VT, Custom);
  }
  setOperationAction(ISD::SHL_PARTS, MVT::i16, Expand);
  setOperationAction(ISD::SRL_PARTS, MVT::i16, Expand);
  setOperationAction(ISD::SRA_PARTS, MVT::i16, Expand);

  // Control flow: unconditional branch via the BR pseudo. Compares fold into
  // BR_CC, which we custom-lower to an I4004ISD::BRCC (SUB chain + JCN). SETCC
  // and SELECT_CC are expanded so they funnel through BR_CC / branch selects.
  setOperationAction(ISD::BR_CC, MVT::i8, Custom);
  setOperationAction(ISD::BR_CC, MVT::i16, Custom);
  setOperationAction(ISD::SETCC, MVT::i8, Expand);
  setOperationAction(ISD::SETCC, MVT::i16, Expand);
  setOperationAction(ISD::SELECT, MVT::i8, Expand);
  setOperationAction(ISD::SELECT, MVT::i16, Expand);
  // SELECT_CC -> custom: a SELECT pseudo expanded to a CFG diamond by
  // EmitInstrWithCustomInserter (the standard small-target pattern).
  setOperationAction(ISD::SELECT_CC, MVT::i8, Custom);
  setOperationAction(ISD::SELECT_CC, MVT::i16, Custom);
  setOperationAction(ISD::BRCOND, MVT::Other, Expand);
  setOperationAction(ISD::BR_JT, MVT::Other, Expand);
  setOperationAction(ISD::BRIND, MVT::Other, Expand);
}

const char *I4004TargetLowering::getTargetNodeName(unsigned Opcode) const {
  switch (Opcode) {
  case I4004ISD::RET:
    return "I4004ISD::RET";
  case I4004ISD::CALL:
    return "I4004ISD::CALL";
  case I4004ISD::WRAPPER:
    return "I4004ISD::WRAPPER";
  default:
    return nullptr;
  }
}

SDValue I4004TargetLowering::LowerOperation(SDValue Op,
                                            SelectionDAG &DAG) const {
  switch (Op.getOpcode()) {
  case ISD::GlobalAddress:
    return LowerGlobalAddress(Op, DAG);
  case ISD::BR_CC:
    return LowerBR_CC(Op, DAG);
  case ISD::SELECT_CC:
    return LowerSELECT_CC(Op, DAG);
  case ISD::AND:
  case ISD::OR:
  case ISD::XOR:
  case ISD::SHL:
  case ISD::SRL:
  case ISD::SRA:
    return LowerBitwise(Op, DAG);
  default:
    llvm_unreachable("unimplemented operation lowering");
  }
}

// Nibble mask of the value bits `Zero` proves zero: bit i set iff nibble i is
// known all-zero.
static unsigned zeroNibbles(const APInt &Zero, unsigned NumNibs) {
  unsigned M = 0;
  for (unsigned i = 0; i < NumNibs; ++i)
    if (Zero.extractBitsAsZExtValue(4, 4 * i) == 0xF)
      M |= 1u << i;
  return M;
}

// Native lowering of the bitwise/shift idioms the 4004 does in a few nibble
// instructions (the machine's unit is the 4-bit register, so a byte mask or a
// shift by 4 is register renaming, not arithmetic):
//   op x, C        (AND/OR/XOR)  -> LOGICI: per-nibble copy / constant / CMA /
//                                   CY-ring bit edit
//   shift x, C                   -> SHIFTI: nibble moves + RAL/RAR chains, when
//                                   no bigger than the libcall site
//   or/xor x, y with every nibble known zero on one side
//                                -> NIBSEL: nibble moves only
// Returns an empty SDValue when none applies (the caller emits the libcall).
SDValue I4004TargetLowering::LowerNibbleOp(SDValue Op,
                                           SelectionDAG &DAG) const {
  EVT VT = Op.getValueType();
  const unsigned Bits = VT.getSizeInBits();
  const unsigned NumNibs = Bits / 4;
  const uint64_t ValMask = (Bits == 16) ? 0xFFFF : 0xFF;
  SDLoc DL(Op);
  SDValue LHS = Op.getOperand(0), RHS = Op.getOperand(1);
  auto TC = [&](uint64_t V) { return DAG.getTargetConstant(V, DL, MVT::i16); };
  const unsigned Opc = Op.getOpcode();

  if (auto *C = dyn_cast<ConstantSDNode>(RHS)) {
    uint64_t Imm = C->getZExtValue();
    if (Opc == ISD::AND || Opc == ISD::OR || Opc == ISD::XOR) {
      unsigned LOp = Opc == ISD::AND  ? I4004Nib::AND
                     : Opc == ISD::OR ? I4004Nib::OR
                                      : I4004Nib::XOR;
      unsigned KZ = zeroNibbles(DAG.computeKnownBits(LHS).Zero, NumNibs);
      return DAG.getNode(I4004ISD::LOGICI, DL, VT, LHS, TC(LOp | (KZ << 4)),
                         TC(Imm & ValMask));
    }
    // Shifts by a constant. Amt >= width is poison: any value will do.
    if (Imm >= Bits)
      return DAG.getUNDEF(VT);
    if (Imm == 0)
      return LHS;
    unsigned Kind = Opc == ISD::SHL   ? I4004Nib::SHL
                    : Opc == ISD::SRL ? I4004Nib::SRL
                                      : I4004Nib::SRA;
    if (I4004Nib::shiftInlineCost(NumNibs, Kind, Imm) <=
        I4004Nib::ShiftInlineMax)
      return DAG.getNode(I4004ISD::SHIFTI, DL, VT, LHS, TC(Kind), TC(Imm));
    return SDValue();
  }

  if (Opc == ISD::OR || Opc == ISD::XOR) {
    KnownBits KL = DAG.computeKnownBits(LHS);
    KnownBits KR = DAG.computeKnownBits(RHS);
    unsigned ZL = zeroNibbles(KL.Zero, NumNibs);
    unsigned ZR = zeroNibbles(KR.Zero, NumNibs);
    const unsigned All = (1u << NumNibs) - 1;
    if ((ZL | ZR) == All) {
      // Nibble i comes from rhs where lhs is known zero (else from lhs).
      unsigned Sel = ZL & ~ZR;
      unsigned Both = ZL & ZR;
      // Look through a constant AND whose mask keeps every nibble taken from
      // it: `(b & 0xF0) | v` reads b's high nibble straight from b, and the
      // AND that only existed to clear the other nibble goes dead.
      auto Bypass = [&](SDValue V, unsigned Taken) -> SDValue {
        unsigned Opc = V.getOpcode();
        uint64_t M;
        if (Opc == ISD::AND && isa<ConstantSDNode>(V.getOperand(1)))
          M = V.getConstantOperandVal(1);
        else if (Opc == I4004ISD::LOGICI &&
                 (V.getConstantOperandVal(1) & 3) == I4004Nib::AND)
          M = V.getConstantOperandVal(2);
        else
          return V;
        for (unsigned i = 0; i < NumNibs; ++i)
          if ((Taken & (1u << i)) && ((M >> (4 * i)) & 0xF) != 0xF)
            return V;
        return V.getOperand(0);
      };
      LHS = Bypass(LHS, All & ~Sel & ~Both);
      RHS = Bypass(RHS, Sel);
      return DAG.getNode(I4004ISD::NIBSEL, DL, VT, LHS, RHS,
                         TC(Sel | (Both << 4)));
    }
  }
  return SDValue();
}

void I4004TargetLowering::computeKnownBitsForTargetNode(
    const SDValue Op, KnownBits &Known, const APInt &DemandedElts,
    const SelectionDAG &DAG, unsigned Depth) const {
  const unsigned Bits = Op.getValueType().getSizeInBits();
  Known.resetAll();
  switch (Op.getOpcode()) {
  default:
    return;
  case I4004ISD::RAM_RD:
  case I4004ISD::RAM_RS:
    // A 4002 character, zero-extended into the pair.
    if (Op.getResNo() == 0)
      Known.Zero.setBitsFrom(4);
    return;
  case I4004ISD::LOGICI: {
    Known = DAG.computeKnownBits(Op.getOperand(0), Depth + 1);
    KnownBits C = KnownBits::makeConstant(
        APInt(Bits, Op.getConstantOperandVal(2)));
    switch (Op.getConstantOperandVal(1) & 3) {
    case I4004Nib::AND: Known &= C; break;
    case I4004Nib::OR:  Known |= C; break;
    default:            Known ^= C; break;
    }
    return;
  }
  case I4004ISD::SHIFTI: {
    KnownBits Src = DAG.computeKnownBits(Op.getOperand(0), Depth + 1);
    KnownBits Amt = KnownBits::makeConstant(
        APInt(Bits, Op.getConstantOperandVal(2)));
    switch (Op.getConstantOperandVal(1)) {
    case I4004Nib::SHL: Known = KnownBits::shl(Src, Amt); break;
    case I4004Nib::SRL: Known = KnownBits::lshr(Src, Amt); break;
    default:            Known = KnownBits::ashr(Src, Amt); break;
    }
    return;
  }
  case I4004ISD::NIBSEL: {
    KnownBits KL = DAG.computeKnownBits(Op.getOperand(0), Depth + 1);
    KnownBits KR = DAG.computeKnownBits(Op.getOperand(1), Depth + 1);
    uint64_t Sel = Op.getConstantOperandVal(2);
    APInt FromR(Bits, 0), BothZero(Bits, 0);
    for (unsigned i = 0; i < Bits / 4; ++i) {
      if (Sel & (1u << i))
        FromR.setBits(4 * i, 4 * i + 4);
      if (Sel & (0x10u << i))
        BothZero.setBits(4 * i, 4 * i + 4);
    }
    Known.Zero = (KL.Zero & ~FromR) | (KR.Zero & FromR) | BothZero;
    Known.One = ((KL.One & ~FromR) | (KR.One & FromR)) & ~BothZero;
    return;
  }
  }
}

// AND/OR/XOR/shifts the nibble lowering cannot do -> a call into libi4004rt.
// i8 uses __{and,or,xor}qi3 (two i8 args, i8 result); i16 the hi3 variants.
SDValue I4004TargetLowering::LowerBitwise(SDValue Op, SelectionDAG &DAG) const {
  if (SDValue N = LowerNibbleOp(Op, DAG))
    return N;
  EVT VT = Op.getValueType();
  // `a | b` with no common bit set is `a + b`: a native nibble-add chain
  // instead of the bit-serial libcall (e.g. a 0/1 flag ORed into a lamp
  // nibble selected from constants).
  if (Op.getOpcode() == ISD::OR &&
      (Op->getFlags().hasDisjoint() ||
       DAG.haveNoCommonBitsSet(Op.getOperand(0), Op.getOperand(1))))
    return DAG.getNode(ISD::ADD, SDLoc(Op), VT, Op.getOperand(0),
                       Op.getOperand(1));
  const bool Wide = (VT == MVT::i16);
  const char *Name = nullptr;
  switch (Op.getOpcode()) {
  case ISD::AND: Name = Wide ? "__andhi3" : "__andqi3"; break;
  case ISD::OR:  Name = Wide ? "__orhi3"  : "__orqi3";  break;
  case ISD::XOR: Name = Wide ? "__xorhi3" : "__xorqi3"; break;
  case ISD::SHL: Name = Wide ? "__ashlhi3" : "__ashlqi3"; break;
  case ISD::SRL: Name = Wide ? "__lshrhi3" : "__lshrqi3"; break;
  case ISD::SRA: Name = Wide ? "__ashrhi3" : "__ashrqi3"; break;
  default: llvm_unreachable("not a bitwise/shift op");
  }

  SDLoc DL(Op);
  SmallVector<SDValue, 2> Ops{Op.getOperand(0), Op.getOperand(1)};
  SDValue Callee = DAG.getExternalSymbol(Name, getPointerTy(DAG.getDataLayout()));
  TargetLowering::ArgListTy Args;
  for (SDValue V : Ops) {
    TargetLowering::ArgListEntry Entry(V, VT.getTypeForEVT(*DAG.getContext()));
    Entry.IsZExt = true;
    Args.push_back(Entry);
  }
  TargetLowering::CallLoweringInfo CLI(DAG);
  CLI.setDebugLoc(DL)
      .setChain(DAG.getEntryNode())
      .setLibCallee(CallingConv::C, VT.getTypeForEVT(*DAG.getContext()), Callee,
                    std::move(Args));
  return LowerCallTo(CLI).first;
}

// Map an IR CondCode to our I4004CC, swapping LHS/RHS for the strict/swapped
// relations so only EQ/NE/ULT/UGE/SLT/SGE remain. Signed relations get their
// own codes: the firmware's quotient/sqrt code tests signed i16 (`< 0`,
// `.max(0)`), so approximating them by unsigned miscompiles it.
static int mapCC(ISD::CondCode CC, SDValue &LHS, SDValue &RHS) {
  switch (CC) {
  case ISD::SETEQ:  return I4004CC::EQ;
  case ISD::SETNE:  return I4004CC::NE;
  case ISD::SETULT: return I4004CC::ULT;
  case ISD::SETUGE: return I4004CC::UGE;
  case ISD::SETUGT: std::swap(LHS, RHS); return I4004CC::ULT;
  case ISD::SETULE: std::swap(LHS, RHS); return I4004CC::UGE;
  case ISD::SETLT:  return I4004CC::SLT;
  case ISD::SETGE:  return I4004CC::SGE;
  case ISD::SETGT:  std::swap(LHS, RHS); return I4004CC::SLT;
  case ISD::SETLE:  std::swap(LHS, RHS); return I4004CC::SGE;
  default:
    report_fatal_error("I4004: unsupported condition code");
  }
}

// Fold "only the low nibbles can differ" into an EQ/NE/unsigned condition
// code: the nibbles above the highest one not known zero on BOTH sides are
// equal, so the compare chain skips them (issue #18; 4002 RAM reads are
// zero-extended nibbles, so a digit test becomes a 1-nibble compare).
static int withUsedNibbles(int Code, SDValue L, SDValue R, SelectionDAG &DAG) {
  if (Code == I4004CC::SLT || Code == I4004CC::SGE)
    return Code;
  const unsigned Bits = L.getValueType().getSizeInBits();
  APInt Z = DAG.computeKnownBits(L).Zero & DAG.computeKnownBits(R).Zero;
  unsigned N = Bits / 4;
  while (N > 1 && Z.extractBitsAsZExtValue(4, 4 * (N - 1)) == 0xF)
    --N;
  return N == Bits / 4 ? Code : (Code | (N << 4));
}

SDValue I4004TargetLowering::LowerBR_CC(SDValue Op, SelectionDAG &DAG) const {
  SDValue Chain = Op.getOperand(0);
  ISD::CondCode CC = cast<CondCodeSDNode>(Op.getOperand(1))->get();
  SDValue LHS = Op.getOperand(2);
  SDValue RHS = Op.getOperand(3);
  SDValue Dest = Op.getOperand(4);
  SDLoc DL(Op);

  // Map the IR condition to our BRCC code (helper handles canonicalization).
  int Code = withUsedNibbles(mapCC(CC, LHS, RHS), LHS, RHS, DAG);

  SDValue CCVal = DAG.getTargetConstant(Code, DL, MVT::i8);
  return DAG.getNode(I4004ISD::BRCC, DL, MVT::Other, Chain, LHS, RHS, CCVal,
                     Dest);
}

SDValue I4004TargetLowering::LowerSELECT_CC(SDValue Op,
                                            SelectionDAG &DAG) const {
  SDValue LHS = Op.getOperand(0);
  SDValue RHS = Op.getOperand(1);
  SDValue TVal = Op.getOperand(2);
  SDValue FVal = Op.getOperand(3);
  ISD::CondCode CC = cast<CondCodeSDNode>(Op.getOperand(4))->get();
  SDLoc DL(Op);

  int Code = withUsedNibbles(mapCC(CC, LHS, RHS), LHS, RHS, DAG);
  SDValue CCVal = DAG.getTargetConstant(Code, DL, MVT::i8);
  // (selectcc lhs, rhs, tval, fval, cc) -> SELECT pseudo (custom inserter).
  return DAG.getNode(I4004ISD::SELECT_CC, DL, Op.getValueType(), LHS, RHS, TVal,
                     FVal, CCVal);
}

SDValue I4004TargetLowering::LowerGlobalAddress(SDValue Op,
                                                SelectionDAG &DAG) const {
  SDLoc DL(Op);
  const GlobalValue *GV = cast<GlobalAddressSDNode>(Op)->getGlobal();
  EVT PtrVT = getPointerTy(DAG.getDataLayout(), /*AS=*/0);
  SDValue TGA = DAG.getTargetGlobalAddress(GV, DL, PtrVT);
  return DAG.getNode(I4004ISD::WRAPPER, DL, PtrVT, TGA);
}

//===----------------------------------------------------------------------===//
// Formal arguments / return / call.
//===----------------------------------------------------------------------===//

// Widening a promoted argument or result honours zeroext/signext, so the
// receiving side may assert it (AssertZext/AssertSext above).
static unsigned extendFor(const ISD::ArgFlagsTy &F) {
  return F.isZExt() ? ISD::ZERO_EXTEND
                    : F.isSExt() ? ISD::SIGN_EXTEND : ISD::ANY_EXTEND;
}

SDValue I4004TargetLowering::LowerFormalArguments(
    SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::InputArg> &Ins, const SDLoc &DL,
    SelectionDAG &DAG, SmallVectorImpl<SDValue> &InVals) const {
  MachineFunction &MF = DAG.getMachineFunction();
  MachineRegisterInfo &RegInfo = MF.getRegInfo();

  SmallVector<CCValAssign, 16> ArgLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, ArgLocs, *DAG.getContext());
  CCInfo.AnalyzeFormalArguments(Ins, CC_I4004);

  for (CCValAssign &VA : ArgLocs) {
    if (VA.isRegLoc()) {
      const TargetRegisterClass *RC = VA.getLocVT() == MVT::i16
                                          ? (const TargetRegisterClass *)&I4004::DR16RegClass
                                          : (const TargetRegisterClass *)&I4004::PR8RegClass;
      Register VReg = RegInfo.createVirtualRegister(RC);
      RegInfo.addLiveIn(VA.getLocReg(), VReg);
      SDValue ArgValue = DAG.getCopyFromReg(Chain, DL, VReg, VA.getLocVT());
      if (VA.getLocVT() != VA.getValVT()) {
        // A zeroext/signext argument arrives already extended (abi.md 4.1):
        // say so, so a `bool` is not masked again (issue #18).
        const ISD::ArgFlagsTy &F = Ins[VA.getValNo()].Flags;
        if (F.isZExt() || F.isSExt())
          ArgValue = DAG.getNode(F.isZExt() ? ISD::AssertZext : ISD::AssertSext,
                                 DL, VA.getLocVT(), ArgValue,
                                 DAG.getValueType(VA.getValVT()));
        ArgValue = DAG.getNode(ISD::TRUNCATE, DL, VA.getValVT(), ArgValue);
      }
      InVals.push_back(ArgValue);
    } else {
      // Stack argument: a RAM-backed slot (abi.md sec.4.1). Backbone loads it
      // from a frame index; real 4002 placement is a later milestone.
      assert(VA.isMemLoc() && "unexpected arg location");
      int FI = MF.getFrameInfo().CreateFixedObject(1, VA.getLocMemOffset(),
                                                   /*IsImmutable=*/true);
      EVT PtrVT = getPointerTy(DAG.getDataLayout());
      SDValue FIN = DAG.getFrameIndex(FI, PtrVT);
      SDValue Load = DAG.getLoad(VA.getLocVT(), DL, Chain, FIN,
                                 MachinePointerInfo::getFixedStack(MF, FI));
      InVals.push_back(Load);
    }
  }
  return Chain;
}

SDValue I4004TargetLowering::LowerReturn(
    SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::OutputArg> &Outs,
    const SmallVectorImpl<SDValue> &OutVals, const SDLoc &DL,
    SelectionDAG &DAG) const {
  MachineFunction &MF = DAG.getMachineFunction();

  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, RVLocs, *DAG.getContext());
  CCInfo.AnalyzeReturn(Outs, RetCC_I4004);

  SDValue Glue;
  SmallVector<SDValue, 4> RetOps(1, Chain);

  for (unsigned i = 0, e = RVLocs.size(); i != e; ++i) {
    CCValAssign &VA = RVLocs[i];
    assert(VA.isRegLoc() && "return value must go in a register");
    SDValue Val = OutVals[i];
    if (VA.getLocVT() != VA.getValVT())
      Val = DAG.getNode(extendFor(Outs[VA.getValNo()].Flags), DL, VA.getLocVT(),
                        Val);
    Chain = DAG.getCopyToReg(Chain, DL, VA.getLocReg(), Val, Glue);
    Glue = Chain.getValue(1);
    RetOps.push_back(DAG.getRegister(VA.getLocReg(), VA.getLocVT()));
  }

  RetOps[0] = Chain;
  if (Glue.getNode())
    RetOps.push_back(Glue);

  return DAG.getNode(I4004ISD::RET, DL, MVT::Other, RetOps);
}

SDValue
I4004TargetLowering::LowerCall(TargetLowering::CallLoweringInfo &CLI,
                               SmallVectorImpl<SDValue> &InVals) const {
  SelectionDAG &DAG = CLI.DAG;
  SDLoc &DL = CLI.DL;
  SmallVectorImpl<ISD::OutputArg> &Outs = CLI.Outs;
  SmallVectorImpl<SDValue> &OutVals = CLI.OutVals;
  SmallVectorImpl<ISD::InputArg> &Ins = CLI.Ins;
  SDValue Chain = CLI.Chain;
  SDValue Callee = CLI.Callee;
  CallingConv::ID CallConv = CLI.CallConv;
  bool &IsTailCall = CLI.IsTailCall;
  IsTailCall = false;

  MachineFunction &MF = DAG.getMachineFunction();

  // 4002 RAM intrinsics (issue #18, C4): the runtime names __ram_rd/__ram_wr/
  // __ram_rs<k>/__ram_ws<k> are not calls but single SRC + I/O instructions.
  StringRef CalleeName;
  if (auto *G = dyn_cast<GlobalAddressSDNode>(Callee))
    CalleeName = G->getGlobal()->getName();
  else if (auto *E = dyn_cast<ExternalSymbolSDNode>(Callee))
    CalleeName = E->getSymbol();
  if (CalleeName.starts_with("__ram_")) {
    auto Imm = [&](unsigned K) { return DAG.getTargetConstant(K, DL, MVT::i8); };
    auto Arg = [&](unsigned I) {
      SDValue V = OutVals[I];
      return V.getValueType() == MVT::i8 ? V
                                         : DAG.getNode(ISD::TRUNCATE, DL, MVT::i8, V);
    };
    SDVTList RdVTs = DAG.getVTList(MVT::i8, MVT::Other);
    if (CalleeName == "__ram_rd") {
      SDValue N = DAG.getNode(I4004ISD::RAM_RD, DL, RdVTs, Chain, Arg(0));
      InVals.push_back(N);
      return N.getValue(1);
    }
    if (CalleeName == "__ram_wr")
      return DAG.getNode(I4004ISD::RAM_WR, DL, MVT::Other, Chain, Arg(0), Arg(1));
    // __ram_rs0..3 / __ram_ws0..3
    if (CalleeName.size() == 9 && CalleeName[8] >= '0' && CalleeName[8] <= '3') {
      unsigned K = CalleeName[8] - '0';
      if (CalleeName.starts_with("__ram_rs")) {
        SDValue N =
            DAG.getNode(I4004ISD::RAM_RS, DL, RdVTs, Chain, Arg(0), Imm(K));
        InVals.push_back(N);
        return N.getValue(1);
      }
      if (CalleeName.starts_with("__ram_ws"))
        return DAG.getNode(I4004ISD::RAM_WS, DL, MVT::Other, Chain, Arg(0),
                           Arg(1), Imm(K));
    }
    report_fatal_error("I4004: unknown RAM intrinsic " + CalleeName);
  }

  SmallVector<CCValAssign, 16> ArgLocs;
  CCState CCInfo(CallConv, CLI.IsVarArg, MF, ArgLocs, *DAG.getContext());
  CCInfo.AnalyzeCallOperands(Outs, CC_I4004);

  unsigned NumBytes = CCInfo.getStackSize();
  Chain = DAG.getCALLSEQ_START(Chain, NumBytes, 0, DL);

  SmallVector<std::pair<Register, SDValue>, 4> RegsToPass;
  SmallVector<SDValue, 4> MemOpChains;
  for (unsigned i = 0, e = ArgLocs.size(); i != e; ++i) {
    CCValAssign &VA = ArgLocs[i];
    SDValue Arg = OutVals[i];
    if (VA.getLocVT() != VA.getValVT())
      Arg = DAG.getNode(extendFor(Outs[VA.getValNo()].Flags), DL, VA.getLocVT(),
                        Arg);
    if (VA.isRegLoc()) {
      RegsToPass.push_back(std::make_pair(VA.getLocReg(), Arg));
    } else {
      // Stack argument: store into the outgoing-arg RAM area via a fixed frame
      // object (abi.md sec.4.1; non-reentrant firmware shares the area).
      assert(VA.isMemLoc() && "unexpected arg location");
      MachineFunction &MF = DAG.getMachineFunction();
      int FI = MF.getFrameInfo().CreateFixedObject(
          VA.getLocVT() == MVT::i16 ? 2 : 1, VA.getLocMemOffset(),
          /*IsImmutable=*/false);
      SDValue FIN = DAG.getFrameIndex(FI, getPointerTy(DAG.getDataLayout()));
      MemOpChains.push_back(
          DAG.getStore(Chain, DL, Arg, FIN,
                       MachinePointerInfo::getFixedStack(MF, FI)));
    }
  }
  if (!MemOpChains.empty())
    Chain = DAG.getNode(ISD::TokenFactor, DL, MVT::Other, MemOpChains);

  SDValue Glue;
  for (auto &R : RegsToPass) {
    Chain = DAG.getCopyToReg(Chain, DL, R.first, R.second, Glue);
    Glue = Chain.getValue(1);
  }

  if (GlobalAddressSDNode *G = dyn_cast<GlobalAddressSDNode>(Callee))
    Callee = DAG.getTargetGlobalAddress(G->getGlobal(), DL, MVT::i16);
  else if (ExternalSymbolSDNode *E = dyn_cast<ExternalSymbolSDNode>(Callee))
    Callee = DAG.getTargetExternalSymbol(E->getSymbol(), MVT::i16);

  SmallVector<SDValue, 8> Ops;
  Ops.push_back(Chain);
  Ops.push_back(Callee);
  for (auto &R : RegsToPass)
    Ops.push_back(DAG.getRegister(R.first, R.second.getValueType()));
  // What the call clobbers: everything but P6 (abi.md 4.3), or, for a
  // hand-written `__leaf_*` routine or the HAL's `__bus_*`, everything but
  // P4-P6 -- those touch only P0-P3 (abi.md 4.3.1).
  const bool LightCallee =
      CalleeName.starts_with("__leaf_") || CalleeName.starts_with("__bus_");
  const uint32_t *Mask =
      LightCallee ? I4004RegisterInfo::getLeafCallPreservedMask()
                  : Subtarget.getRegisterInfo()->getCallPreservedMask(MF, CallConv);
  Ops.push_back(DAG.getRegisterMask(Mask));
  if (Glue.getNode())
    Ops.push_back(Glue);

  SDVTList NodeTys = DAG.getVTList(MVT::Other, MVT::Glue);
  Chain = DAG.getNode(I4004ISD::CALL, DL, NodeTys, Ops);
  Glue = Chain.getValue(1);

  Chain = DAG.getCALLSEQ_END(Chain, NumBytes, 0, Glue, DL);
  Glue = Chain.getValue(1);

  return LowerCallResult(Chain, Glue, CallConv, CLI.IsVarArg, Ins, DL, DAG,
                         InVals);
}

SDValue I4004TargetLowering::LowerCallResult(
    SDValue Chain, SDValue InGlue, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::InputArg> &Ins, const SDLoc &DL,
    SelectionDAG &DAG, SmallVectorImpl<SDValue> &InVals) const {
  MachineFunction &MF = DAG.getMachineFunction();
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, RVLocs, *DAG.getContext());
  CCInfo.AnalyzeCallResult(Ins, RetCC_I4004);

  for (CCValAssign &VA : RVLocs) {
    SDValue Val =
        DAG.getCopyFromReg(Chain, DL, VA.getLocReg(), VA.getLocVT(), InGlue);
    Chain = Val.getValue(1);
    InGlue = Val.getValue(2);
    if (VA.getLocVT() != VA.getValVT()) {
      // The callee extended a zeroext/signext result (abi.md 4.2).
      const ISD::ArgFlagsTy &F = Ins[VA.getValNo()].Flags;
      if (F.isZExt() || F.isSExt())
        Val = DAG.getNode(F.isZExt() ? ISD::AssertZext : ISD::AssertSext, DL,
                          VA.getLocVT(), Val, DAG.getValueType(VA.getValVT()));
      Val = DAG.getNode(ISD::TRUNCATE, DL, VA.getValVT(), Val);
    }
    InVals.push_back(Val);
  }
  return Chain;
}

//===----------------------------------------------------------------------===//
// Custom inserter: expand SELECTi8/SELECTi16 into a compare-and-branch CFG
// diamond with a PHI. Operands: dst = SELECT lhs, rhs, cc, tval, fval.
//===----------------------------------------------------------------------===//
MachineBasicBlock *
I4004TargetLowering::EmitInstrWithCustomInserter(MachineInstr &MI,
                                                 MachineBasicBlock *BB) const {
  const TargetInstrInfo &TII = *Subtarget.getInstrInfo();
  DebugLoc DL = MI.getDebugLoc();
  const BasicBlock *LLVM_BB = BB->getBasicBlock();
  MachineFunction::iterator It = ++BB->getIterator();

  Register Dst = MI.getOperand(0).getReg();
  Register Lhs = MI.getOperand(1).getReg();
  Register Rhs = MI.getOperand(2).getReg();
  int64_t CC = MI.getOperand(3).getImm();
  Register TVal = MI.getOperand(4).getReg();
  Register FVal = MI.getOperand(5).getReg();

  // Pick the compare width from the LHS operand's register class: a DR16 (i16)
  // compare uses BRCCi16, otherwise BRCCi8. Operands are virtual at this stage.
  MachineFunction *MF = BB->getParent();
  const MachineRegisterInfo &MRI = MF->getRegInfo();
  const TargetRegisterClass *LhsRC = MRI.getRegClass(Lhs);
  bool CmpWide = I4004::DR16RegClass.hasSubClassEq(LhsRC);
  unsigned BrOpc = CmpWide ? I4004::BRCCi16 : I4004::BRCCi8;
  // thisMBB: compare lhs,rhs; if cc true branch to sinkMBB taking TVal (via the
  // PHI's thisMBB edge), else fall through to copyMBB.
  MachineBasicBlock *ThisMBB = BB;
  MachineBasicBlock *CopyMBB = MF->CreateMachineBasicBlock(LLVM_BB);
  MachineBasicBlock *SinkMBB = MF->CreateMachineBasicBlock(LLVM_BB);
  MF->insert(It, CopyMBB);
  MF->insert(It, SinkMBB);

  // Move everything after MI in ThisMBB to SinkMBB.
  SinkMBB->splice(SinkMBB->begin(), ThisMBB,
                  std::next(MachineBasicBlock::iterator(MI)), ThisMBB->end());
  SinkMBB->transferSuccessorsAndUpdatePHIs(ThisMBB);

  ThisMBB->addSuccessor(CopyMBB);
  ThisMBB->addSuccessor(SinkMBB);

  // ThisMBB: BRCC lhs, rhs, cc, SinkMBB  (taken => use TVal).
  BuildMI(ThisMBB, DL, TII.get(BrOpc))
      .addReg(Lhs)
      .addReg(Rhs)
      .addImm(CC)
      .addMBB(SinkMBB);

  // CopyMBB: fall through to SinkMBB (carries FVal).
  CopyMBB->addSuccessor(SinkMBB);

  // SinkMBB: dst = PHI [TVal, ThisMBB], [FVal, CopyMBB].
  BuildMI(*SinkMBB, SinkMBB->begin(), DL, TII.get(TargetOpcode::PHI), Dst)
      .addReg(TVal)
      .addMBB(ThisMBB)
      .addReg(FVal)
      .addMBB(CopyMBB);

  MI.eraseFromParent();
  return SinkMBB;
}

bool I4004TargetLowering::decomposeMulByConstant(LLVMContext &, EVT VT,
                                                 SDValue C) const {
  if (VT != MVT::i8 && VT != MVT::i16)
    return false;
  auto *N = dyn_cast<ConstantSDNode>(C);
  if (!N)
    return false;
  // A negative factor costs one more negation: decompose |c|.
  APInt Imm = N->getAPIntValue();
  if (Imm.isNegative())
    Imm.negate();
  if (Imm.isZero())
    return false;
  // Native shifts: up to 2 places on i8, 1 on i16 (longer ones are libcalls).
  const unsigned MaxShift = VT == MVT::i8 ? 2 : 1;
  const unsigned M = Imm.countr_zero();
  const APInt K = Imm.lshr(M);
  auto Small = [MaxShift](const APInt &V) {
    return V.isPowerOf2() && V.logBase2() <= MaxShift;
  };
  // (An i16 factor must be odd: the DAG may fold 2^m into a longer shift.)
  const unsigned MaxM = VT == MVT::i8 ? 2 : 0;
  return M <= MaxM && (Small(K - 1) || Small(K + 1));
}
