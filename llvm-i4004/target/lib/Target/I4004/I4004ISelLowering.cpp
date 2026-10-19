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
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include "llvm/CodeGen/SelectionDAG.h"
#include "llvm/CodeGen/TargetLoweringObjectFileImpl.h"
#include "llvm/IR/GlobalValue.h"
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

  // i8 ALU the 4004 lacks natively -> libcalls (libi4004rt, abi.md sec.6) or
  // expand. AND/OR/XOR are expanded via nibble sequences in the post-RA pass
  // (no native bitwise ALU); here they are Expand so DAG tries the generic
  // route, with i8 MUL/DIV/shift as libcalls.
  setOperationAction(ISD::MUL, MVT::i8, LibCall);
  setOperationAction(ISD::SDIV, MVT::i8, LibCall);
  setOperationAction(ISD::UDIV, MVT::i8, LibCall);
  setOperationAction(ISD::SREM, MVT::i8, LibCall);
  setOperationAction(ISD::UREM, MVT::i8, LibCall);
  setOperationAction(ISD::SHL, MVT::i8, LibCall);
  setOperationAction(ISD::SRL, MVT::i8, LibCall);
  setOperationAction(ISD::SRA, MVT::i8, LibCall);
  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::ROTL, VT, Expand);
    setOperationAction(ISD::ROTR, VT, Expand);
  }

  // i16 arithmetic/bitwise: expand to i8 ops on the two sub-pairs. ADD/SUB
  // expand to ADDC/SUBC chains over the legal i8 (which our ADDi8/SUBi8
  // pseudos select). MUL/DIV/shift -> libcalls.
  setOperationAction(ISD::MUL, MVT::i16, LibCall);
  setOperationAction(ISD::SDIV, MVT::i16, LibCall);
  setOperationAction(ISD::UDIV, MVT::i16, LibCall);
  setOperationAction(ISD::SREM, MVT::i16, LibCall);
  setOperationAction(ISD::UREM, MVT::i16, LibCall);
  setOperationAction(ISD::SHL, MVT::i16, LibCall);
  setOperationAction(ISD::SRL, MVT::i16, LibCall);
  setOperationAction(ISD::SRA, MVT::i16, LibCall);

  // AND/OR/XOR: the 4004 has no native bitwise ALU. Expand so the DAG breaks
  // them down; the actual nibble-table / libcall lowering is a later step.
  for (MVT VT : {MVT::i8, MVT::i16}) {
    setOperationAction(ISD::AND, VT, Expand);
    setOperationAction(ISD::OR, VT, Expand);
    setOperationAction(ISD::XOR, VT, Expand);
  }

  setMinFunctionAlignment(Align(1));
  setPrefFunctionAlignment(Align(1));

  // Control flow (this increment): unconditional branch works via the BR
  // pseudo. Compares/conditional branches are Expand here and lowered to the
  // 4004's ACC==0 / CY test in the next increment (SETCC/BR_CC custom).
  setOperationAction(ISD::BR_CC, MVT::i8, Expand);
  setOperationAction(ISD::BR_CC, MVT::i16, Expand);
  setOperationAction(ISD::SELECT_CC, MVT::i8, Expand);
  setOperationAction(ISD::SELECT_CC, MVT::i16, Expand);
  setOperationAction(ISD::BRCOND, MVT::Other, Legal);
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
  default:
    llvm_unreachable("unimplemented operation lowering");
  }
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
  for (unsigned i = 0, e = ArgLocs.size(); i != e; ++i) {
    CCValAssign &VA = ArgLocs[i];
    SDValue Arg = OutVals[i];
    if (VA.getLocVT() != VA.getValVT())
      Arg = DAG.getNode(ISD::ANY_EXTEND, DL, VA.getLocVT(), Arg);
    assert(VA.isRegLoc() && "backbone supports register args only");
    RegsToPass.push_back(std::make_pair(VA.getLocReg(), Arg));
  }

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
