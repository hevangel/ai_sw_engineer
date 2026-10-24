//===-- I8008ISelLowering.cpp - I8008 DAG Lowering Implementation --------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// spec/codegen.md sec.2 (selection) and abi.md sec.3 (calls).
//
//===----------------------------------------------------------------------===//

#include "I8008ISelLowering.h"
#include "I8008.h"
#include "I8008MachineFunctionInfo.h"
#include "I8008RegisterInfo.h"
#include "I8008Subtarget.h"
#include "I8008TargetMachine.h"
#include "llvm/CodeGen/CallingConvLower.h"
#include "llvm/CodeGen/MachineFrameInfo.h"
#include "llvm/CodeGen/MachineFunction.h"
#include "llvm/CodeGen/MachineInstrBuilder.h"
#include "llvm/CodeGen/MachineRegisterInfo.h"
#include "llvm/CodeGen/SelectionDAG.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/GlobalVariable.h"
#include "llvm/IR/Module.h"

using namespace llvm;

#define DEBUG_TYPE "i8008-lower"

#define GET_SDNODE_ENUM
#include "I8008GenSDNodeInfo.inc"

#define GET_CALLING_CONV_IMPL
#include "I8008GenCallingConv.inc"

I8008TargetLowering::I8008TargetLowering(const TargetMachine &TM,
                                         const I8008Subtarget &STI)
    : TargetLowering(TM, STI), Subtarget(STI) {
  addRegisterClass(MVT::i8, &I8008::GR8RegClass);
  addRegisterClass(MVT::i16, &I8008::GR16RegClass);
  computeRegisterProperties(STI.getRegisterInfo());

  setBooleanContents(ZeroOrOneBooleanContent);
  setMinFunctionAlignment(Align(1));
  setPrefFunctionAlignment(Align(1));
  // No indirect jump (isa.md): never build jump tables.
  setMinimumJumpTableEntries(UINT_MAX);

  for (MVT VT : {MVT::i8, MVT::i16}) {
    for (unsigned Op : {ISD::GlobalAddress, ISD::ExternalSymbol,
                        ISD::BlockAddress})
      setOperationAction(Op, MVT::i16, Custom);
    setOperationAction(ISD::BR_CC, VT, Custom);
    setOperationAction(ISD::SELECT_CC, VT, Custom);
    setOperationAction(ISD::SETCC, VT, Expand);
    setOperationAction(ISD::SELECT, VT, Expand);
    setOperationAction(ISD::SHL, VT, Custom);
    setOperationAction(ISD::SRL, VT, Custom);
    setOperationAction(ISD::SRA, VT, Custom);
    for (unsigned Op :
         {ISD::ROTL, ISD::ROTR, ISD::BSWAP, ISD::CTPOP, ISD::CTLZ, ISD::CTTZ,
          ISD::MULHU, ISD::MULHS, ISD::UMUL_LOHI, ISD::SMUL_LOHI,
          ISD::SDIVREM, ISD::UDIVREM, ISD::UADDO, ISD::USUBO, ISD::SADDO,
          ISD::SSUBO, ISD::UADDO_CARRY, ISD::USUBO_CARRY, ISD::ABS})
      setOperationAction(Op, VT, Expand);
    for (unsigned Op : {ISD::MUL, ISD::SDIV, ISD::UDIV, ISD::SREM, ISD::UREM})
      setOperationAction(Op, VT, LibCall);
  }
  for (unsigned Op : {ISD::MUL, ISD::SDIV, ISD::UDIV, ISD::SREM, ISD::UREM})
    setOperationAction(Op, MVT::i32, LibCall);
  for (unsigned Op : {ISD::SHL_PARTS, ISD::SRL_PARTS, ISD::SRA_PARTS})
    setOperationAction(Op, MVT::i16, Expand);

  setOperationAction(ISD::BRCOND, MVT::Other, Expand);
  setOperationAction(ISD::BR_JT, MVT::Other, Expand);
  setOperationAction(ISD::BRIND, MVT::Other, Expand);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i8, Expand);
  setOperationAction(ISD::SIGN_EXTEND_INREG, MVT::i16, Expand);
  setOperationAction(ISD::DYNAMIC_STACKALLOC, MVT::i16, Expand);
  setOperationAction(ISD::STACKSAVE, MVT::Other, Expand);
  setOperationAction(ISD::STACKRESTORE, MVT::Other, Expand);
  setOperationAction(ISD::ATOMIC_FENCE, MVT::Other, Expand);

  // Only i8 and i16 memory accesses; everything else goes through them.
  for (MVT VT : MVT::integer_valuetypes()) {
    setLoadExtAction(ISD::EXTLOAD, VT, MVT::i1, Promote);
    setLoadExtAction(ISD::ZEXTLOAD, VT, MVT::i1, Promote);
    setLoadExtAction(ISD::SEXTLOAD, VT, MVT::i1, Promote);
    setLoadExtAction(ISD::EXTLOAD, VT, MVT::i8, Expand);
    setLoadExtAction(ISD::ZEXTLOAD, VT, MVT::i8, Expand);
    setLoadExtAction(ISD::SEXTLOAD, VT, MVT::i8, Expand);
  }
  setTruncStoreAction(MVT::i16, MVT::i8, Expand);

  // Small constant-size memcpy/memset become loads and stores.
  MaxStoresPerMemcpy = MaxStoresPerMemcpyOptSize = 4;
  MaxStoresPerMemset = MaxStoresPerMemsetOptSize = 4;
  MaxStoresPerMemmove = MaxStoresPerMemmoveOptSize = 4;
}

const char *I8008TargetLowering::getTargetNodeName(unsigned Opcode) const {
  switch (Opcode) {
  case I8008ISD::RET: return "I8008ISD::RET";
  case I8008ISD::CALL: return "I8008ISD::CALL";
  case I8008ISD::WRAPPER: return "I8008ISD::WRAPPER";
  case I8008ISD::BRCC: return "I8008ISD::BRCC";
  case I8008ISD::SELECT_CC: return "I8008ISD::SELECT_CC";
  default: return nullptr;
  }
}

SDValue I8008TargetLowering::LowerOperation(SDValue Op,
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
  case ISD::SHL:
  case ISD::SRL:
  case ISD::SRA:
    return LowerShift(Op, DAG);
  default:
    llvm_unreachable("unimplemented operation lowering");
  }
}

SDValue I8008TargetLowering::LowerAddress(SDValue Op, SelectionDAG &DAG) const {
  SDLoc DL(Op);
  SDValue T;
  if (auto *G = dyn_cast<GlobalAddressSDNode>(Op))
    T = DAG.getTargetGlobalAddress(G->getGlobal(), DL, MVT::i16, G->getOffset());
  else if (auto *E = dyn_cast<ExternalSymbolSDNode>(Op))
    T = DAG.getTargetExternalSymbol(E->getSymbol(), MVT::i16);
  else
    T = DAG.getTargetBlockAddress(cast<BlockAddressSDNode>(Op)->getBlockAddress(),
                                  MVT::i16);
  return DAG.getNode(I8008ISD::WRAPPER, DL, MVT::i16, T);
}

static int mapCC(ISD::CondCode CC) {
  switch (CC) {
  case ISD::SETEQ: return I8008CC::EQ;
  case ISD::SETNE: return I8008CC::NE;
  case ISD::SETULT: return I8008CC::ULT;
  case ISD::SETUGE: return I8008CC::UGE;
  case ISD::SETUGT: return I8008CC::UGT;
  case ISD::SETULE: return I8008CC::ULE;
  case ISD::SETLT: return I8008CC::SLT;
  case ISD::SETGE: return I8008CC::SGE;
  case ISD::SETGT: return I8008CC::SGT;
  case ISD::SETLE: return I8008CC::SLE;
  default:
    report_fatal_error("I8008: unsupported condition code");
  }
}

// A constant operand goes on the right (the BRCC*ri patterns want it there).
static int canonicalCC(ISD::CondCode CC, SDValue &LHS, SDValue &RHS) {
  int Code = mapCC(CC);
  if (isa<ConstantSDNode>(LHS) && !isa<ConstantSDNode>(RHS)) {
    std::swap(LHS, RHS);
    Code = I8008CC::swapped(Code);
  }
  return Code;
}

SDValue I8008TargetLowering::LowerBR_CC(SDValue Op, SelectionDAG &DAG) const {
  SDValue LHS = Op.getOperand(2), RHS = Op.getOperand(3);
  int Code = canonicalCC(cast<CondCodeSDNode>(Op.getOperand(1))->get(), LHS, RHS);
  SDLoc DL(Op);
  return DAG.getNode(I8008ISD::BRCC, DL, MVT::Other, Op.getOperand(0), LHS, RHS,
                     DAG.getTargetConstant(Code, DL, MVT::i8),
                     Op.getOperand(4));
}

SDValue I8008TargetLowering::LowerSELECT_CC(SDValue Op,
                                            SelectionDAG &DAG) const {
  SDValue LHS = Op.getOperand(0), RHS = Op.getOperand(1);
  int Code = canonicalCC(cast<CondCodeSDNode>(Op.getOperand(4))->get(), LHS, RHS);
  SDLoc DL(Op);
  return DAG.getNode(I8008ISD::SELECT_CC, DL, Op.getValueType(), LHS, RHS,
                     Op.getOperand(2), Op.getOperand(3),
                     DAG.getTargetConstant(Code, DL, MVT::i8));
}

// Constant shifts the expander does inline stay as they are (the SHL/SRL/SRA
// patterns take them); everything else is a call into rt/shift.s.
SDValue I8008TargetLowering::LowerShift(SDValue Op, SelectionDAG &DAG) const {
  EVT VT = Op.getValueType();
  const bool Wide = VT == MVT::i16;
  const unsigned Bits = VT.getSizeInBits();
  SDLoc DL(Op);
  if (auto *C = dyn_cast<ConstantSDNode>(Op.getOperand(1))) {
    uint64_t K = C->getZExtValue();
    if (K == 0)
      return Op.getOperand(0);
    if (K >= Bits)
      return DAG.getUNDEF(VT);
    // i8: at most 7 one-byte steps. i16: a byte move plus a few steps.
    if (!Wide || K <= 3 || K >= 8)
      return Op;
  }
  const char *Name;
  switch (Op.getOpcode()) {
  case ISD::SHL: Name = Wide ? "__ashlhi3" : "__ashlqi3"; break;
  case ISD::SRL: Name = Wide ? "__lshrhi3" : "__lshrqi3"; break;
  default:       Name = Wide ? "__ashrhi3" : "__ashrqi3"; break;
  }
  SDValue Amt = DAG.getZExtOrTrunc(Op.getOperand(1), DL, MVT::i8);
  LLVMContext &Ctx = *DAG.getContext();
  TargetLowering::ArgListTy Args;
  Args.emplace_back(Op.getOperand(0), VT.getTypeForEVT(Ctx));
  Args.emplace_back(Amt, Type::getInt8Ty(Ctx));
  TargetLowering::CallLoweringInfo CLI(DAG);
  CLI.setDebugLoc(DL)
      .setChain(DAG.getEntryNode())
      .setLibCallee(CallingConv::C, VT.getTypeForEVT(Ctx),
                    DAG.getExternalSymbol(Name, MVT::i16), std::move(Args));
  return LowerCallTo(CLI).first;
}

//===----------------------------------------------------------------------===//
// Calls (abi.md sec.3)
//===----------------------------------------------------------------------===//

SDValue I8008TargetLowering::argArea(SelectionDAG &DAG, const SDLoc &DL,
                                     StringRef Fn, int64_t Off) const {
  Module &M = *DAG.getMachineFunction().getFunction().getParent();
  std::string Name = ("__args_" + Fn).str();
  GlobalVariable *GV = M.getNamedGlobal(Name);
  if (!GV)
    GV = new GlobalVariable(M, Type::getInt8Ty(M.getContext()),
                            /*isConstant=*/false, GlobalValue::ExternalLinkage,
                            nullptr, Name);
  return DAG.getNode(I8008ISD::WRAPPER, DL, MVT::i16,
                     DAG.getTargetGlobalAddress(GV, DL, MVT::i16, Off));
}

static unsigned extendFor(const ISD::ArgFlagsTy &F) {
  return F.isZExt() ? ISD::ZERO_EXTEND
                    : F.isSExt() ? ISD::SIGN_EXTEND : ISD::ANY_EXTEND;
}

static const TargetRegisterClass *classFor(MVT VT) {
  return VT == MVT::i16 ? &I8008::GR16RegClass : &I8008::GR8RegClass;
}

SDValue I8008TargetLowering::LowerFormalArguments(
    SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::InputArg> &Ins, const SDLoc &DL,
    SelectionDAG &DAG, SmallVectorImpl<SDValue> &InVals) const {
  MachineFunction &MF = DAG.getMachineFunction();
  if (IsVarArg)
    report_fatal_error("I8008: variadic functions are not supported");
  SmallVector<CCValAssign, 16> ArgLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, ArgLocs, *DAG.getContext());
  CCInfo.AnalyzeFormalArguments(Ins, CC_I8008);
  MF.getInfo<I8008MachineFunctionInfo>()->ArgAreaSize = CCInfo.getStackSize();

  for (CCValAssign &VA : ArgLocs) {
    SDValue V;
    if (VA.isRegLoc()) {
      Register VReg = MF.getRegInfo().createVirtualRegister(classFor(VA.getLocVT()));
      MF.getRegInfo().addLiveIn(VA.getLocReg(), VReg);
      V = DAG.getCopyFromReg(Chain, DL, VReg, VA.getLocVT());
    } else {
      SDValue Addr = argArea(DAG, DL, MF.getName(), VA.getLocMemOffset());
      V = DAG.getLoad(VA.getLocVT(), DL, Chain, Addr, MachinePointerInfo());
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

bool I8008TargetLowering::CanLowerReturn(
    CallingConv::ID CallConv, MachineFunction &MF, bool IsVarArg,
    const SmallVectorImpl<ISD::OutputArg> &Outs, LLVMContext &Context,
    const Type *RetTy) const {
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, RVLocs, Context);
  return CCInfo.CheckReturn(Outs, RetCC_I8008);
}

SDValue I8008TargetLowering::LowerReturn(
    SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::OutputArg> &Outs,
    const SmallVectorImpl<SDValue> &OutVals, const SDLoc &DL,
    SelectionDAG &DAG) const {
  MachineFunction &MF = DAG.getMachineFunction();
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, MF, RVLocs, *DAG.getContext());
  CCInfo.AnalyzeReturn(Outs, RetCC_I8008);

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
  return DAG.getNode(I8008ISD::RET, DL, MVT::Other, RetOps);
}

SDValue I8008TargetLowering::LowerCall(TargetLowering::CallLoweringInfo &CLI,
                                       SmallVectorImpl<SDValue> &InVals) const {
  SelectionDAG &DAG = CLI.DAG;
  SDLoc &DL = CLI.DL;
  SDValue Chain = CLI.Chain, Callee = CLI.Callee;
  CLI.IsTailCall = false;
  MachineFunction &MF = DAG.getMachineFunction();
  if (CLI.IsVarArg)
    report_fatal_error("I8008: variadic calls are not supported");

  StringRef CalleeName;
  if (auto *G = dyn_cast<GlobalAddressSDNode>(Callee))
    CalleeName = G->getGlobal()->getName();
  else if (auto *E = dyn_cast<ExternalSymbolSDNode>(Callee))
    CalleeName = E->getSymbol();

  SmallVector<CCValAssign, 16> ArgLocs;
  CCState CCInfo(CLI.CallConv, CLI.IsVarArg, MF, ArgLocs, *DAG.getContext());
  CCInfo.AnalyzeCallOperands(CLI.Outs, CC_I8008);
  unsigned NumBytes = CCInfo.getStackSize();
  if (NumBytes && CalleeName.empty())
    report_fatal_error("I8008: an indirect call cannot pass memory arguments");

  Chain = DAG.getCALLSEQ_START(Chain, 0, 0, DL);

  SmallVector<std::pair<Register, SDValue>, 4> RegsToPass;
  SmallVector<SDValue, 4> Stores;
  for (unsigned I = 0; I < ArgLocs.size(); ++I) {
    CCValAssign &VA = ArgLocs[I];
    SDValue Arg = CLI.OutVals[I];
    if (VA.getLocVT() != VA.getValVT())
      Arg = DAG.getNode(extendFor(CLI.Outs[VA.getValNo()].Flags), DL,
                        VA.getLocVT(), Arg);
    if (VA.isRegLoc()) {
      RegsToPass.push_back({VA.getLocReg(), Arg});
    } else {
      SDValue Addr = argArea(DAG, DL, CalleeName, VA.getLocMemOffset());
      Stores.push_back(DAG.getStore(Chain, DL, Arg, Addr, MachinePointerInfo()));
    }
  }
  if (!Stores.empty())
    Chain = DAG.getNode(ISD::TokenFactor, DL, MVT::Other, Stores);

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
  // A `__leaf_*` callee keeps C D E H L (and B when it returns nothing).
  const uint32_t *Mask =
      CalleeName.starts_with("__leaf_")
          ? (CLI.Ins.empty() ? I8008RegisterInfo::leafVoidMask()
                             : I8008RegisterInfo::leafMask())
          : Subtarget.getRegisterInfo()->getCallPreservedMask(MF, CLI.CallConv);
  Ops.push_back(DAG.getRegisterMask(Mask));
  if (Glue.getNode())
    Ops.push_back(Glue);

  Chain = DAG.getNode(I8008ISD::CALL, DL, DAG.getVTList(MVT::Other, MVT::Glue),
                      Ops);
  Glue = Chain.getValue(1);
  Chain = DAG.getCALLSEQ_END(Chain, 0, 0, Glue, DL);
  Glue = Chain.getValue(1);
  return LowerCallResult(Chain, Glue, CLI.CallConv, CLI.IsVarArg, CLI.Ins, DL,
                         DAG, InVals);
}

SDValue I8008TargetLowering::LowerCallResult(
    SDValue Chain, SDValue InGlue, CallingConv::ID CallConv, bool IsVarArg,
    const SmallVectorImpl<ISD::InputArg> &Ins, const SDLoc &DL,
    SelectionDAG &DAG, SmallVectorImpl<SDValue> &InVals) const {
  SmallVector<CCValAssign, 16> RVLocs;
  CCState CCInfo(CallConv, IsVarArg, DAG.getMachineFunction(), RVLocs,
                 *DAG.getContext());
  CCInfo.AnalyzeCallResult(Ins, RetCC_I8008);
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

bool I8008TargetLowering::decomposeMulByConstant(LLVMContext &, EVT VT,
                                                 SDValue C) const {
  auto *N = dyn_cast<ConstantSDNode>(C);
  if (!N || (VT != MVT::i8 && VT != MVT::i16))
    return false;
  APInt Imm = N->getAPIntValue();
  if (Imm.isNegative())
    Imm.negate();
  auto Small = [](const APInt &V) { return V.isPowerOf2() && V.logBase2() <= 4; };
  return Small(Imm) || Small(Imm - 1) || Small(Imm + 1);
}

//===----------------------------------------------------------------------===//
// SELECT_* -> compare-and-branch diamond with a PHI.
//===----------------------------------------------------------------------===//

MachineBasicBlock *
I8008TargetLowering::EmitInstrWithCustomInserter(MachineInstr &MI,
                                                 MachineBasicBlock *BB) const {
  const TargetInstrInfo &TII = *Subtarget.getInstrInfo();
  DebugLoc DL = MI.getDebugLoc();
  MachineFunction *MF = BB->getParent();
  const BasicBlock *LLVMBB = BB->getBasicBlock();
  MachineFunction::iterator It = ++BB->getIterator();

  Register Dst = MI.getOperand(0).getReg();
  Register Lhs = MI.getOperand(1).getReg();
  Register Rhs = MI.getOperand(2).getReg();
  int64_t CC = MI.getOperand(3).getImm();
  Register TVal = MI.getOperand(4).getReg();
  Register FVal = MI.getOperand(5).getReg();
  MachineRegisterInfo &MRI = MF->getRegInfo();
  bool Wide = I8008::GR16RegClass.hasSubClassEq(MRI.getRegClass(Lhs));
  // A constant right-hand side compares as an immediate (BRCC*ri), so it
  // needs no register; its MOV*ri becomes dead.
  const MachineInstr *RDef = MRI.getVRegDef(Rhs);
  bool RImm = RDef && (RDef->getOpcode() == I8008::MOV8ri ||
                       RDef->getOpcode() == I8008::MOV16ri);

  MachineBasicBlock *CopyMBB = MF->CreateMachineBasicBlock(LLVMBB);
  MachineBasicBlock *SinkMBB = MF->CreateMachineBasicBlock(LLVMBB);
  MF->insert(It, CopyMBB);
  MF->insert(It, SinkMBB);
  SinkMBB->splice(SinkMBB->begin(), BB,
                  std::next(MachineBasicBlock::iterator(MI)), BB->end());
  SinkMBB->transferSuccessorsAndUpdatePHIs(BB);
  BB->addSuccessor(CopyMBB);
  BB->addSuccessor(SinkMBB);
  auto Br = BuildMI(BB, DL,
                    TII.get(RImm ? (Wide ? I8008::BRCC16ri : I8008::BRCC8ri)
                                 : (Wide ? I8008::BRCC16rr : I8008::BRCC8rr)))
                .addReg(Lhs);
  if (RImm)
    Br.addImm(RDef->getOperand(1).getImm());
  else
    Br.addReg(Rhs);
  Br.addImm(CC).addMBB(SinkMBB);
  CopyMBB->addSuccessor(SinkMBB);
  BuildMI(*SinkMBB, SinkMBB->begin(), DL, TII.get(TargetOpcode::PHI), Dst)
      .addReg(TVal)
      .addMBB(BB)
      .addReg(FVal)
      .addMBB(CopyMBB);
  MI.eraseFromParent();
  return SinkMBB;
}
