//===-- I8086ISelLowering.cpp - I8086 DAG Lowering Implementation ----------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// spec/codegen.md (selection) and spec/abi.md D3 (calls).
//
//===----------------------------------------------------------------------===//

#include "I8086ISelLowering.h"
#include "I8086.h"
#include "I8086InstrInfo.h"
#include "I8086MachineFunctionInfo.h"
#include "I8086RegisterInfo.h"
#include "I8086Subtarget.h"
#include "I8086TargetMachine.h"
#include "llvm/CodeGen/CallingConvLower.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include "llvm/CodeGen/SelectionDAG.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/Module.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-lower"

#define GET_SDNODE_ENUM
#include "I8086GenSDNodeInfo.inc"

#define GET_CALLING_CONV_IMPL
#include "I8086GenCallingConv.inc"

I8086TargetLowering::I8086TargetLowering(const TargetMachine &TM,
                                         const I8086Subtarget &STI)
    : TargetLowering(TM, STI), Subtarget(STI) {
  addRegisterClass(MVT::i8, &I8086::GR8RegClass);
  addRegisterClass(MVT::i16, &I8086::GR16RegClass);
  computeRegisterProperties(STI.getRegisterInfo());

  setStackPointerRegisterToSaveRestore(I8086::SP);
  setBooleanContents(ZeroOrOneBooleanContent);
  setMinFunctionAlignment(Align(1));
  setPrefFunctionAlignment(Align(1));
  // No jump tables: an indirect jump needs the table in the same segment and
  // a bounds check that costs more than a short compare chain.
  setMinimumJumpTableEntries(UINT_MAX);

  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::BR_CC, VT, Custom);
    setOperationAction(ISD::SELECT_CC, VT, Custom);
    setOperationAction(ISD::SETCC, VT, Custom);
    setOperationAction(ISD::SELECT, VT, Expand);
    for (unsigned Op :
         {ISD::ROTL, ISD::ROTR, ISD::BSWAP, ISD::CTPOP, ISD::CTLZ, ISD::CTTZ,
          ISD::MULHU, ISD::MULHS, ISD::UMUL_LOHI, ISD::SMUL_LOHI,
          ISD::SDIVREM, ISD::UDIVREM, ISD::UADDO, ISD::USUBO, ISD::SADDO,
          ISD::SSUBO, ISD::UADDO_CARRY, ISD::USUBO_CARRY, ISD::ABS,
          ISD::UMULO, ISD::SMULO, ISD::SHL_PARTS, ISD::SRL_PARTS,
          ISD::SRA_PARTS})
      setOperationAction(Op, VT, Expand);
  }
  for (unsigned Op : {ISD::GlobalAddress, ISD::ExternalSymbol,
                      ISD::BlockAddress})
    setOperationAction(Op, MVT::i16, Custom);

  // 16-bit multiply and divide are the native mul/imul/div/idiv on AX and DX
  // (selected in I8086DAGToDAGISel::Select); the 8-bit forms are promoted to
  // 16 bits. A 32-bit multiply is built from the 16-bit full products; 32-bit
  // divide and remainder and variable 32-bit shifts are runtime calls
  // (abi.md, Runtime library).
  for (unsigned Op : {ISD::MUL, ISD::SDIV, ISD::UDIV, ISD::SREM, ISD::UREM})
    setOperationAction(Op, MVT::i8, Promote);
  setOperationAction(ISD::MUL, MVT::i16, Legal);
  setOperationAction(ISD::UMUL_LOHI, MVT::i16, Legal);
  setOperationAction(ISD::SMUL_LOHI, MVT::i16, Legal);
  setOperationAction(ISD::UDIVREM, MVT::i16, Legal);
  setOperationAction(ISD::SDIVREM, MVT::i16, Legal);
  for (unsigned Op : {ISD::SDIV, ISD::UDIV, ISD::SREM, ISD::UREM})
    setOperationAction(Op, MVT::i16, Expand);
  setOperationAction(ISD::MUL, MVT::i32, Expand);
  for (unsigned Op : {ISD::SDIV, ISD::UDIV, ISD::SREM, ISD::UREM})
    setOperationAction(Op, MVT::i32, LibCall);
  for (unsigned Op : {ISD::MUL, ISD::SDIV, ISD::UDIV, ISD::SREM, ISD::UREM})
    setOperationAction(Op, MVT::i64, LibCall);
  for (unsigned Op : {ISD::SHL, ISD::SRL, ISD::SRA}) {
    setOperationAction(Op, MVT::i32, LibCall);
    setOperationAction(Op, MVT::i64, LibCall);
  }

  // Add and subtract with carry: the carry travels in the flags between the
  // 16-bit pieces of a wider value.
  for (unsigned Op : {ISD::UADDO, ISD::USUBO, ISD::UADDO_CARRY,
                      ISD::USUBO_CARRY})
    setOperationAction(Op, MVT::i16, Custom);

  setOperationAction(ISD::BRCOND, MVT::Other, Expand);
  setOperationAction(ISD::BR_JT, MVT::Other, Expand);
  setOperationAction(ISD::BRIND, MVT::Other, Expand);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i1, Expand);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i8, Custom);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i16, Expand);
  setOperationAction(ISD::DYNAMIC_STACKALLOC, MVT::i16, Expand);
  setOperationAction(ISD::STACKSAVE, MVT::Other, Expand);
  setOperationAction(ISD::STACKRESTORE, MVT::Other, Expand);
  setOperationAction(ISD::ATOMIC_FENCE, MVT::Other, Expand);

  // Only i8 and i16 memory accesses; wider and narrower ones go through them.
  for (MVT VT : MVT::integer_valuetypes()) {
    setLoadExtAction(ISD::EXTLOAD, VT, MVT::i1, Promote);
    setLoadExtAction(ISD::ZEXTLOAD, VT, MVT::i1, Promote);
    setLoadExtAction(ISD::SEXTLOAD, VT, MVT::i1, Promote);
    setLoadExtAction(ISD::EXTLOAD, VT, MVT::i8, Expand);
    setLoadExtAction(ISD::ZEXTLOAD, VT, MVT::i8, Expand);
    setLoadExtAction(ISD::SEXTLOAD, VT, MVT::i8, Expand);
  }
  setTruncStoreAction(MVT::i16, MVT::i8, Expand);

  // Small constant-size memory operations become loads and stores.
  MaxStoresPerMemcpy = MaxStoresPerMemcpyOptSize = 4;
  MaxStoresPerMemset = MaxStoresPerMemsetOptSize = 4;
  MaxStoresPerMemmove = MaxStoresPerMemmoveOptSize = 4;
}

const char *I8086TargetLowering::getTargetNodeName(unsigned Opcode) const {
  switch (Opcode) {
  case I8086ISD::RET: return "I8086ISD::RET";
  case I8086ISD::CALL: return "I8086ISD::CALL";
  case I8086ISD::WRAPPER: return "I8086ISD::WRAPPER";
  case I8086ISD::PUSH: return "I8086ISD::PUSH";
  case I8086ISD::ADDC: return "I8086ISD::ADDC";
  case I8086ISD::ADDE: return "I8086ISD::ADDE";
  case I8086ISD::SUBC: return "I8086ISD::SUBC";
  case I8086ISD::SUBE: return "I8086ISD::SUBE";
  case I8086ISD::REP_MOVSB: return "I8086ISD::REP_MOVSB";
  case I8086ISD::REP_MOVSW: return "I8086ISD::REP_MOVSW";
  case I8086ISD::REP_STOSB: return "I8086ISD::REP_STOSB";
  case I8086ISD::REP_STOSW: return "I8086ISD::REP_STOSW";
  case I8086ISD::CMP: return "I8086ISD::CMP";
  case I8086ISD::BRCC: return "I8086ISD::BRCC";
  case I8086ISD::SELECT_CC: return "I8086ISD::SELECT_CC";
  default: return nullptr;
  }
}

SDValue I8086TargetLowering::LowerOperation(SDValue Op,
                                            SelectionDAG &DAG) const {
  switch (Op.getOpcode()) {
  case ISD::GlobalAddress:
  case ISD::ExternalSymbol:
  case ISD::BlockAddress:
    return LowerAddress(Op, DAG);
  case ISD::BR_CC:
    return LowerBR_CC(Op, DAG);
  case ISD::SELECT_CC:
    return LowerSELECT_CC(Op, DAG);
  case ISD::SETCC:
    return LowerSETCC(Op, DAG);
  case ISD::SIGN_EXTEND_INREG:
    return LowerSIGN_EXTEND_INREG(Op, DAG);
  case ISD::UADDO:
  case ISD::USUBO:
  case ISD::UADDO_CARRY:
  case ISD::USUBO_CARRY:
    return LowerAddSubCarry(Op, DAG);
  default:
    llvm_unreachable("unexpected custom lowering");
  }
}

SDValue I8086TargetLowering::LowerAddress(SDValue Op, SelectionDAG &DAG) const {
  SDLoc DL(Op);
  SDValue T;
  if (auto *G = dyn_cast<GlobalAddressSDNode>(Op))
    T = DAG.getTargetGlobalAddress(G->getGlobal(), DL, MVT::i16, G->getOffset());
  else if (auto *E = dyn_cast<ExternalSymbolSDNode>(Op))
    T = DAG.getTargetExternalSymbol(E->getSymbol(), MVT::i16);
  else
    T = DAG.getTargetBlockAddress(cast<BlockAddressSDNode>(Op)->getBlockAddress(),
                                  MVT::i16);
  return DAG.getNode(I8086ISD::WRAPPER, DL, MVT::i16, T);
}

//===----------------------------------------------------------------------===//
// Comparisons and selects
//===----------------------------------------------------------------------===//

// The hardware condition code (the low nibble of the Jcc opcode) that holds
// after `cmp lhs, rhs` exactly when CC does.
static int hardwareCC(ISD::CondCode CC) {
  switch (CC) {
  case ISD::SETEQ:  return I8086CC::E;
  case ISD::SETNE:  return I8086CC::NE;
  case ISD::SETULT: return I8086CC::B;
  case ISD::SETUGE: return I8086CC::AE;
  case ISD::SETULE: return I8086CC::BE;
  case ISD::SETUGT: return I8086CC::A;
  case ISD::SETLT:  return I8086CC::L;
  case ISD::SETGE:  return I8086CC::GE;
  case ISD::SETLE:  return I8086CC::LE;
  case ISD::SETGT:  return I8086CC::G;
  default:
    llvm_unreachable("unsupported condition code");
  }
}

// The compare node and the hardware code; a constant on the left is moved to
// the right (there is no `cmp imm, reg`).
static SDValue emitCmp(SDValue LHS, SDValue RHS, ISD::CondCode CC,
                       const SDLoc &DL, SelectionDAG &DAG, int &HW) {
  if (isa<ConstantSDNode>(LHS) && !isa<ConstantSDNode>(RHS)) {
    std::swap(LHS, RHS);
    CC = ISD::getSetCCSwappedOperands(CC);
  }
  HW = hardwareCC(CC);
  return DAG.getNode(I8086ISD::CMP, DL, MVT::Glue, LHS, RHS);
}

SDValue I8086TargetLowering::LowerBR_CC(SDValue Op, SelectionDAG &DAG) const {
  SDValue Chain = Op.getOperand(0);
  ISD::CondCode CC = cast<CondCodeSDNode>(Op.getOperand(1))->get();
  SDValue LHS = Op.getOperand(2), RHS = Op.getOperand(3), Dest = Op.getOperand(4);
  SDLoc DL(Op);
  int HW;
  SDValue Cmp = emitCmp(LHS, RHS, CC, DL, DAG, HW);
  return DAG.getNode(I8086ISD::BRCC, DL, MVT::Other, Chain, Dest,
                     DAG.getTargetConstant(HW, DL, MVT::i8), Cmp);
}

SDValue I8086TargetLowering::LowerSELECT_CC(SDValue Op,
                                            SelectionDAG &DAG) const {
  SDValue LHS = Op.getOperand(0), RHS = Op.getOperand(1);
  SDValue TV = Op.getOperand(2), FV = Op.getOperand(3);
  ISD::CondCode CC = cast<CondCodeSDNode>(Op.getOperand(4))->get();
  SDLoc DL(Op);
  int HW;
  SDValue Cmp = emitCmp(LHS, RHS, CC, DL, DAG, HW);
  return DAG.getNode(I8086ISD::SELECT_CC, DL, Op.getValueType(), TV, FV,
                     DAG.getTargetConstant(HW, DL, MVT::i8), Cmp);
}

// There is no setcc instruction: a boolean is a select between 1 and 0.
SDValue I8086TargetLowering::LowerSETCC(SDValue Op, SelectionDAG &DAG) const {
  SDLoc DL(Op);
  EVT VT = Op.getValueType();
  SDValue Sel = DAG.getNode(
      ISD::SELECT_CC, DL, VT, Op.getOperand(0), Op.getOperand(1),
      DAG.getConstant(1, DL, VT), DAG.getConstant(0, DL, VT), Op.getOperand(2));
  return LowerSELECT_CC(Sel, DAG);
}

// Add / subtract with a carry (borrow) in and out, for the pieces of a wider
// value. The carry out is a boolean made from the flags; a carry in is turned
// back into the flag, unless it is the boolean just made from the previous
// piece's flags (nothing else uses it), in which case those flags are used
// as they are, so a 32-bit add is `add` then `adc`.
SDValue I8086TargetLowering::LowerAddSubCarry(SDValue Op,
                                              SelectionDAG &DAG) const {
  SDLoc DL(Op);
  unsigned Opc = Op.getOpcode();
  bool IsAdd = Opc == ISD::UADDO || Opc == ISD::UADDO_CARRY;
  bool HasIn = Opc == ISD::UADDO_CARRY || Opc == ISD::USUBO_CARRY;
  SDValue A = Op.getOperand(0), B = Op.getOperand(1);

  SDValue Flags;
  if (HasIn) {
    SDValue Carry = Op.getOperand(2);
    auto IsConst = [](SDValue V, uint64_t K) {
      auto *C = dyn_cast<ConstantSDNode>(V);
      return C && C->getZExtValue() == K;
    };
    if (Carry.getOpcode() == I8086ISD::SELECT_CC && Carry.hasOneUse() &&
        IsConst(Carry.getOperand(0), 1) && IsConst(Carry.getOperand(1), 0) &&
        cast<ConstantSDNode>(Carry.getOperand(2))->getZExtValue() ==
            I8086CC::B) {
      Flags = Carry.getOperand(3);
    } else {
      // CF = (carry != 0): adding 0xFF to a 0/1 byte carries exactly when it
      // is 1.
      SDValue Bool = DAG.getZExtOrTrunc(Carry, DL, MVT::i8);
      Flags = DAG.getNode(I8086ISD::ADDC, DL, DAG.getVTList(MVT::i8, MVT::Glue),
                          Bool, DAG.getConstant(0xFF, DL, MVT::i8))
                  .getValue(1);
    }
  }
  unsigned NodeOpc = HasIn ? (IsAdd ? I8086ISD::ADDE : I8086ISD::SUBE)
                           : (IsAdd ? I8086ISD::ADDC : I8086ISD::SUBC);
  SDVTList VTs = DAG.getVTList(MVT::i16, MVT::Glue);
  SDValue Res = HasIn ? DAG.getNode(NodeOpc, DL, VTs, A, B, Flags)
                      : DAG.getNode(NodeOpc, DL, VTs, A, B);
  SDValue CarryOut =
      DAG.getNode(I8086ISD::SELECT_CC, DL, MVT::i8, DAG.getConstant(1, DL, MVT::i8),
                  DAG.getConstant(0, DL, MVT::i8),
                  DAG.getTargetConstant(I8086CC::B, DL, MVT::i8),
                  Res.getValue(1));
  return DAG.getMergeValues({Res.getValue(0), CarryOut}, DL);
}

// Sign-extending the low byte of a word is `cbw` on the byte.
SDValue I8086TargetLowering::LowerSIGN_EXTEND_INREG(SDValue Op,
                                                    SelectionDAG &DAG) const {
  SDLoc DL(Op);
  EVT VT = Op.getValueType();
  assert(cast<VTSDNode>(Op.getOperand(1))->getVT() == MVT::i8 &&
         VT == MVT::i16 && "only i8 inside i16 is custom");
  SDValue Byte = DAG.getNode(ISD::TRUNCATE, DL, MVT::i8, Op.getOperand(0));
  return DAG.getNode(ISD::SIGN_EXTEND, DL, MVT::i16, Byte);
}

//===----------------------------------------------------------------------===//
// Calls (abi.md D3)
//===----------------------------------------------------------------------===//

static unsigned extendFor(const ISD::ArgFlagsTy &F) {
  return F.isZExt() ? ISD::ZERO_EXTEND
                    : F.isSExt() ? ISD::SIGN_EXTEND : ISD::ANY_EXTEND;
}

SDValue I8086TargetLowering::LowerFormalArguments(
    SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::InputArg> &Ins, const SDLoc &DL,
    SelectionDAG &DAG, SmallVectorImpl<SDValue> &InVals) const {
  MachineFunction &MF = DAG.getMachineFunction();
  MachineFrameInfo &MFI = MF.getFrameInfo();
  if (IsVarArg)
    report_fatal_error("I8086: variadic functions are not supported yet");
  SmallVector<CCValAssign, 16> ArgLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, ArgLocs, *DAG.getContext());
  CCInfo.AnalyzeFormalArguments(Ins, CC_I8086);

  for (CCValAssign &VA : ArgLocs) {
    if (Ins[VA.getValNo()].Flags.isByVal())
      report_fatal_error("I8086: byval arguments are not supported yet");
    SDValue V;
    if (VA.isRegLoc()) {
      Register VReg = MF.getRegInfo().createVirtualRegister(&I8086::GR16RegClass);
      MF.getRegInfo().addLiveIn(VA.getLocReg(), VReg);
      V = DAG.getCopyFromReg(Chain, DL, VReg, VA.getLocVT());
    } else {
      // The argument area starts above the saved BP and the return address:
      // [bp + 4 + offset], that is entry-SP offset + 2 (the return address).
      int FI = MFI.CreateFixedObject(2, VA.getLocMemOffset() + 2, true);
      SDValue Addr = DAG.getFrameIndex(FI, MVT::i16);
      V = DAG.getLoad(VA.getLocVT(), DL, Chain, Addr,
                      MachinePointerInfo::getFixedStack(MF, FI));
    }
    if (VA.getLocVT() != VA.getValVT()) {
      const ISD::ArgFlagsTy &F = Ins[VA.getValNo()].Flags;
      if (F.isZExt() || F.isSExt())
        V = DAG.getNode(F.isZExt() ? ISD::AssertZext : ISD::AssertSext, DL,
                        VA.getLocVT(), V, DAG.getValueType(VA.getValVT()));
      V = DAG.getNode(ISD::TRUNCATE, DL, VA.getValVT(), V);
    }
    InVals.push_back(V);
  }
  return Chain;
}

bool I8086TargetLowering::CanLowerReturn(
    CallingConv::ID CallConv, MachineFunction &MF, bool IsVarArg,
    const SmallVectorImpl<ISD::OutputArg> &Outs, LLVMContext &Context,
    const Type *RetTy) const {
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, RVLocs, Context);
  return CCInfo.CheckReturn(Outs, RetCC_I8086);
}

SDValue I8086TargetLowering::LowerReturn(
    SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::OutputArg> &Outs,
    const SmallVectorImpl<SDValue> &OutVals, const SDLoc &DL,
    SelectionDAG &DAG) const {
  MachineFunction &MF = DAG.getMachineFunction();
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, RVLocs, *DAG.getContext());
  CCInfo.AnalyzeReturn(Outs, RetCC_I8086);

  SDValue Glue;
  SmallVector<SDValue, 4> RetOps(1, Chain);
  for (unsigned I = 0; I < RVLocs.size(); ++I) {
    CCValAssign &VA = RVLocs[I];
    SDValue V = OutVals[I];
    if (VA.getLocVT() != VA.getValVT())
      V = DAG.getNode(extendFor(Outs[VA.getValNo()].Flags), DL, VA.getLocVT(), V);
    Chain = DAG.getCopyToReg(Chain, DL, VA.getLocReg(), V, Glue);
    Glue = Chain.getValue(1);
    RetOps.push_back(DAG.getRegister(VA.getLocReg(), VA.getLocVT()));
  }
  RetOps[0] = Chain;
  if (Glue.getNode())
    RetOps.push_back(Glue);
  return DAG.getNode(I8086ISD::RET, DL, MVT::Other, RetOps);
}

SDValue I8086TargetLowering::LowerCall(TargetLowering::CallLoweringInfo &CLI,
                                       SmallVectorImpl<SDValue> &InVals) const {
  SelectionDAG &DAG = CLI.DAG;
  SDLoc &DL = CLI.DL;
  SDValue Chain = CLI.Chain, Callee = CLI.Callee;
  CLI.IsTailCall = false;
  MachineFunction &MF = DAG.getMachineFunction();
  if (CLI.IsVarArg)
    report_fatal_error("I8086: variadic calls are not supported yet");

  SmallVector<CCValAssign, 16> ArgLocs;
  CCState CCInfo(CLI.CallConv, CLI.IsVarArg, MF, ArgLocs, *DAG.getContext());
  CCInfo.AnalyzeCallOperands(CLI.Outs, CC_I8086);
  unsigned NumBytes = CCInfo.getStackSize();

  Chain = DAG.getCALLSEQ_START(Chain, NumBytes, 0, DL);

  auto Promote = [&](unsigned I) {
    CCValAssign &VA = ArgLocs[I];
    SDValue Arg = CLI.OutVals[I];
    if (CLI.Outs[I].Flags.isByVal())
      report_fatal_error("I8086: byval arguments are not supported yet");
    if (VA.getLocVT() != VA.getValVT())
      Arg = DAG.getNode(extendFor(CLI.Outs[VA.getValNo()].Flags), DL,
                        VA.getLocVT(), Arg);
    return Arg;
  };

  // Memory arguments are pushed right to left, so the first is at the lowest
  // address.
  for (unsigned I = ArgLocs.size(); I-- > 0;)
    if (ArgLocs[I].isMemLoc())
      Chain = DAG.getNode(I8086ISD::PUSH, DL, MVT::Other, Chain, Promote(I));

  SmallVector<std::pair<Register, SDValue>, 4> RegsToPass;
  for (unsigned I = 0; I < ArgLocs.size(); ++I)
    if (ArgLocs[I].isRegLoc())
      RegsToPass.push_back({ArgLocs[I].getLocReg(), Promote(I)});

  SDValue Glue;
  for (auto &R : RegsToPass) {
    Chain = DAG.getCopyToReg(Chain, DL, R.first, R.second, Glue);
    Glue = Chain.getValue(1);
  }

  if (auto *G = dyn_cast<GlobalAddressSDNode>(Callee))
    Callee = DAG.getTargetGlobalAddress(G->getGlobal(), DL, MVT::i16);
  else if (auto *E = dyn_cast<ExternalSymbolSDNode>(Callee))
    Callee = DAG.getTargetExternalSymbol(E->getSymbol(), MVT::i16);

  SmallVector<SDValue, 8> Ops{Chain, Callee};
  for (auto &R : RegsToPass)
    Ops.push_back(DAG.getRegister(R.first, R.second.getValueType()));
  Ops.push_back(DAG.getRegisterMask(
      Subtarget.getRegisterInfo()->getCallPreservedMask(MF, CLI.CallConv)));
  if (Glue.getNode())
    Ops.push_back(Glue);

  Chain = DAG.getNode(I8086ISD::CALL, DL, DAG.getVTList(MVT::Other, MVT::Glue),
                      Ops);
  Glue = Chain.getValue(1);
  Chain = DAG.getCALLSEQ_END(Chain, NumBytes, 0, Glue, DL);
  Glue = Chain.getValue(1);
  return LowerCallResult(Chain, Glue, CLI.CallConv, CLI.IsVarArg, CLI.Ins, DL,
                         DAG, InVals);
}

SDValue I8086TargetLowering::LowerCallResult(
    SDValue Chain, SDValue InGlue, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::InputArg> &Ins, const SDLoc &DL,
    SelectionDAG &DAG, SmallVectorImpl<SDValue> &InVals) const {
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, DAG.getMachineFunction(), RVLocs,
                 *DAG.getContext());
  CCInfo.AnalyzeCallResult(Ins, RetCC_I8086);
  for (CCValAssign &VA : RVLocs) {
    SDValue V =
        DAG.getCopyFromReg(Chain, DL, VA.getLocReg(), VA.getLocVT(), InGlue);
    Chain = V.getValue(1);
    InGlue = V.getValue(2);
    if (VA.getLocVT() != VA.getValVT()) {
      const ISD::ArgFlagsTy &F = Ins[VA.getValNo()].Flags;
      if (F.isZExt() || F.isSExt())
        V = DAG.getNode(F.isZExt() ? ISD::AssertZext : ISD::AssertSext, DL,
                        VA.getLocVT(), V, DAG.getValueType(VA.getValVT()));
      V = DAG.getNode(ISD::TRUNCATE, DL, VA.getValVT(), V);
    }
    InVals.push_back(V);
  }
  return Chain;
}

//===----------------------------------------------------------------------===//
// P_SELECT* -> a conditional branch over a copy, joined by a PHI.
//===----------------------------------------------------------------------===//

MachineBasicBlock *
I8086TargetLowering::EmitInstrWithCustomInserter(MachineInstr &MI,
                                                 MachineBasicBlock *BB) const {
  assert((MI.getOpcode() == I8086::P_SELECT8 ||
          MI.getOpcode() == I8086::P_SELECT16) &&
         "unexpected instruction for the custom inserter");
  const TargetInstrInfo &TII = *Subtarget.getInstrInfo();
  DebugLoc DL = MI.getDebugLoc();
  MachineFunction *MF = BB->getParent();
  const BasicBlock *LLVMBB = BB->getBasicBlock();
  MachineFunction::iterator It = ++BB->getIterator();

  Register Dst = MI.getOperand(0).getReg();
  Register TVal = MI.getOperand(1).getReg();
  Register FVal = MI.getOperand(2).getReg();
  int64_t CC = MI.getOperand(3).getImm();

  //  BB:       j<cc> SinkMBB          (the flags were set by the compare)
  //  CopyMBB:  (falls through)
  //  SinkMBB:  Dst = phi [TVal, BB], [FVal, CopyMBB]
  MachineBasicBlock *CopyMBB = MF->CreateMachineBasicBlock(LLVMBB);
  MachineBasicBlock *SinkMBB = MF->CreateMachineBasicBlock(LLVMBB);
  MF->insert(It, CopyMBB);
  MF->insert(It, SinkMBB);
  SinkMBB->splice(SinkMBB->begin(), BB,
                  std::next(MachineBasicBlock::iterator(MI)), BB->end());
  SinkMBB->transferSuccessorsAndUpdatePHIs(BB);
  BB->addSuccessor(CopyMBB);
  BB->addSuccessor(SinkMBB);
  BuildMI(BB, DL, TII.get(I8086::P_JCC)).addMBB(SinkMBB).addImm(CC);
  CopyMBB->addSuccessor(SinkMBB);
  BuildMI(*SinkMBB, SinkMBB->begin(), DL, TII.get(TargetOpcode::PHI), Dst)
      .addReg(TVal)
      .addMBB(BB)
      .addReg(FVal)
      .addMBB(CopyMBB);
  MI.eraseFromParent();
  return SinkMBB;
}

// `add boolean, -1` turns a 0/1 boolean back into the carry flag. When the
// boolean was just made from the flags of the previous add or subtract
// (`SELECT_CC 1, 0, B, flags`) and nothing else uses it, the flags are the
// carry: use them directly, so a wide add is `add` then `adc` with no branch
// in between.
SDValue I8086TargetLowering::PerformDAGCombine(SDNode *N,
                                               DAGCombinerInfo &DCI) const {
  if (N->getOpcode() != I8086ISD::ADDC)
    return SDValue();
  SDValue Bool = N->getOperand(0);
  auto *K = dyn_cast<ConstantSDNode>(N->getOperand(1));
  if (!K || K->getZExtValue() != 0xFF || Bool.getOpcode() != I8086ISD::SELECT_CC ||
      !Bool.hasOneUse() || !SDValue(N, 0).use_empty())
    return SDValue();
  auto IsConst = [](SDValue V, uint64_t Val) {
    auto *C = dyn_cast<ConstantSDNode>(V);
    return C && C->getZExtValue() == Val;
  };
  if (!IsConst(Bool.getOperand(0), 1) || !IsConst(Bool.getOperand(1), 0) ||
      !IsConst(Bool.getOperand(2), I8086CC::B))
    return SDValue();
  return DCI.CombineTo(N, DCI.DAG.getUNDEF(MVT::i8), Bool.getOperand(3));
}
