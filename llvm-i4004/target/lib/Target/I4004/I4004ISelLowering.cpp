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
  // helpers. i8 AND/OR/XOR custom-lower to __{and,or,xor}qi3 (in libi4004rt).
  // i16 AND/OR/XOR also custom-lower, splitting into two i8 qi3 calls on the
  // sub-pairs (no i16 hi3 helper needed).
  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::AND, VT, Custom);
    setOperationAction(ISD::OR, VT, Custom);
    setOperationAction(ISD::XOR, VT, Custom);
  }

  setMinFunctionAlignment(Align(1));
  setPrefFunctionAlignment(Align(1));

  // mul/div/rem libcall impl names are registered in
  // I4004Subtarget::initLibcallLoweringInfo (the LLVM 23 hook).

  // Variable shifts: i8 custom-lower to __{ashl,lshr,ashr}qi3 (in libi4004rt).
  // i16 shifts custom-lower to the hi3 helpers (added to libi4004rt).
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

// AND/OR/XOR and shifts -> libi4004rt calls. i8 bitwise/shift use the qi3
// helpers. An i16 bitwise op is split into two i8 qi3 calls on the low and high
// bytes (no hi3 helper needed); i16 shifts use the hi3 helpers.
SDValue I4004TargetLowering::LowerBitwise(SDValue Op, SelectionDAG &DAG) const {
  EVT VT = Op.getValueType();
  SDLoc DL(Op);
  unsigned Opc = Op.getOpcode();
  const bool IsBitwise = (Opc == ISD::AND || Opc == ISD::OR || Opc == ISD::XOR);

  auto callI8 = [&](const char *Name, SDValue A, SDValue B) -> SDValue {
    SDValue Callee =
        DAG.getExternalSymbol(Name, getPointerTy(DAG.getDataLayout()));
    TargetLowering::ArgListTy Args;
    Type *I8Ty = Type::getInt8Ty(*DAG.getContext());
    for (SDValue V : {A, B}) {
      TargetLowering::ArgListEntry E(V, I8Ty);
      E.IsZExt = true;
      Args.push_back(E);
    }
    TargetLowering::CallLoweringInfo CLI(DAG);
    CLI.setDebugLoc(DL).setChain(DAG.getEntryNode()).setLibCallee(
        CallingConv::C, I8Ty, Callee, std::move(Args));
    return LowerCallTo(CLI).first;
  };

  // i16 bitwise: split into low/high i8 halves, call the qi3 helper on each,
  // and reassemble the i16 from the two i8 results.
  if (VT == MVT::i16 && IsBitwise) {
    const char *Qi = Opc == ISD::AND ? "__andqi3"
                     : Opc == ISD::OR ? "__orqi3"
                                      : "__xorqi3";
    SDValue L = Op.getOperand(0), R = Op.getOperand(1);
    SDValue Llo = DAG.getNode(ISD::TRUNCATE, DL, MVT::i8, L);
    SDValue Rlo = DAG.getNode(ISD::TRUNCATE, DL, MVT::i8, R);
    SDValue Sh = DAG.getConstant(8, DL, MVT::i16);
    SDValue Lhi = DAG.getNode(
        ISD::TRUNCATE, DL, MVT::i8, DAG.getNode(ISD::SRL, DL, MVT::i16, L, Sh));
    SDValue Rhi = DAG.getNode(
        ISD::TRUNCATE, DL, MVT::i8, DAG.getNode(ISD::SRL, DL, MVT::i16, R, Sh));
    SDValue Lo = callI8(Qi, Llo, Rlo);
    SDValue Hi = callI8(Qi, Lhi, Rhi);
    SDValue LoZ = DAG.getNode(ISD::ZERO_EXTEND, DL, MVT::i16, Lo);
    SDValue HiZ = DAG.getNode(ISD::ZERO_EXTEND, DL, MVT::i16, Hi);
    return DAG.getNode(ISD::OR, DL, MVT::i16, LoZ,
                       DAG.getNode(ISD::SHL, DL, MVT::i16, HiZ, Sh));
  }

  const bool Wide = (VT == MVT::i16);
  const char *Name = nullptr;
  switch (Opc) {
  case ISD::AND: Name = "__andqi3"; break;
  case ISD::OR:  Name = "__orqi3";  break;
  case ISD::XOR: Name = "__xorqi3"; break;
  case ISD::SHL: Name = Wide ? "__ashlhi3" : "__ashlqi3"; break;
  case ISD::SRL: Name = Wide ? "__lshrhi3" : "__lshrqi3"; break;
  case ISD::SRA: Name = Wide ? "__ashrhi3" : "__ashrqi3"; break;
  default: llvm_unreachable("not a bitwise/shift op");
  }

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
// relations so only EQ/NE/ULT/UGE remain. Signed relations are approximated by
// unsigned for the backbone (the BCD firmware is unsigned).
static int mapCC(ISD::CondCode CC, SDValue &LHS, SDValue &RHS) {
  switch (CC) {
  case ISD::SETEQ:                   return I4004CC::EQ;
  case ISD::SETNE:                   return I4004CC::NE;
  case ISD::SETULT: case ISD::SETLT: return I4004CC::ULT;
  case ISD::SETUGE: case ISD::SETGE: return I4004CC::UGE;
  case ISD::SETUGT: case ISD::SETGT: std::swap(LHS, RHS); return I4004CC::ULT;
  case ISD::SETULE: case ISD::SETLE: std::swap(LHS, RHS); return I4004CC::UGE;
  default:
    report_fatal_error("I4004: unsupported condition code");
  }
}

SDValue I4004TargetLowering::LowerBR_CC(SDValue Op, SelectionDAG &DAG) const {
  SDValue Chain = Op.getOperand(0);
  ISD::CondCode CC = cast<CondCodeSDNode>(Op.getOperand(1))->get();
  SDValue LHS = Op.getOperand(2);
  SDValue RHS = Op.getOperand(3);
  SDValue Dest = Op.getOperand(4);
  SDLoc DL(Op);

  // Map the IR condition to our BRCC code (helper handles canonicalization).
  int Code = mapCC(CC, LHS, RHS);

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

  int Code = mapCC(CC, LHS, RHS);
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
      if (VA.getLocVT() != VA.getValVT())
        ArgValue = DAG.getNode(ISD::TRUNCATE, DL, VA.getValVT(), ArgValue);
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
      Val = DAG.getNode(ISD::ANY_EXTEND, DL, VA.getLocVT(), Val);
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
      Arg = DAG.getNode(ISD::ANY_EXTEND, DL, VA.getLocVT(), Arg);
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
    if (VA.getLocVT() != VA.getValVT())
      Val = DAG.getNode(ISD::TRUNCATE, DL, VA.getValVT(), Val);
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
