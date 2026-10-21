//===-- I4004SelectionDAGInfo.cpp - I4004 SelectionDAG Info --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004SelectionDAGInfo.h"
#include "llvm/CodeGen/SelectionDAG.h"
#include "llvm/CodeGen/TargetLowering.h"

#define GET_SDNODE_DESC
#include "I4004GenSDNodeInfo.inc"

using namespace llvm;

I4004SelectionDAGInfo::I4004SelectionDAGInfo()
    : SelectionDAGGenTargetInfo(I4004GenSDNodeInfo) {}

I4004SelectionDAGInfo::~I4004SelectionDAGInfo() = default;

// Shared: emit a call  helper(i8 a, i8 b, i8 nibble_count)  where a/b are the
// SRC address bytes (low byte of each DR16 pointer; the high byte selects the
// bank via DCL and is 0 for the Busicom board) and nibble_count = 2 * Size.
// Returns the call chain. Size must be a build-time constant for the x2 to stay
// an i8 the helper's 16-bit-max loop can take; the firmware's mem intrinsics
// all have constant small sizes (<= 20 nibbles), so this holds. A non-constant
// Size falls back to the generic path (return SDValue()).
static SDValue emitRamCall(SelectionDAG &DAG, const SDLoc &dl, SDValue Chain,
                           const char *Name, SDValue DstPtr, SDValue SrcOrVal,
                           SDValue Size, bool SrcIsAddr) {
  auto *CSize = dyn_cast<ConstantSDNode>(Size);
  if (!CSize)
    return SDValue(); // non-constant size -> generic lowering
  const TargetLowering &TLI = DAG.getTargetLoweringInfo();
  LLVMContext &Ctx = *DAG.getContext();
  Type *I8Ty = Type::getInt8Ty(Ctx);
  EVT PtrVT = TLI.getPointerTy(DAG.getDataLayout());

  // dst SRC address byte = low i8 of the pointer.
  SDValue DstByte = DAG.getNode(ISD::TRUNCATE, dl, MVT::i8, DstPtr);
  // src address byte (memcpy/memmove) or fill value (memset): already i8 for
  // memset's value; truncate the pointer for the address case.
  SDValue Arg1 =
      SrcIsAddr ? DAG.getNode(ISD::TRUNCATE, dl, MVT::i8, SrcOrVal) : SrcOrVal;
  if (Arg1.getValueType() != MVT::i8)
    Arg1 = DAG.getNode(ISD::TRUNCATE, dl, MVT::i8, Arg1);
  // nibble count = 2 * byte size (each i8 cell is two 4002 characters).
  uint64_t Nibbles = CSize->getZExtValue() * 2;
  SDValue Count = DAG.getConstant(Nibbles & 0xff, dl, MVT::i8);

  TargetLowering::ArgListTy Args;
  for (SDValue V : {DstByte, Arg1, Count}) {
    TargetLowering::ArgListEntry E(V, I8Ty);
    E.IsZExt = true;
    Args.push_back(E);
  }
  TargetLowering::CallLoweringInfo CLI(DAG);
  CLI.setDebugLoc(dl).setChain(Chain).setLibCallee(
      CallingConv::C, Type::getVoidTy(Ctx),
      DAG.getExternalSymbol(Name, PtrVT), std::move(Args));
  // The helpers return their dst (ignored here); take the chain.
  std::pair<SDValue, SDValue> R = TLI.LowerCallTo(CLI);
  return R.second;
}

SDValue I4004SelectionDAGInfo::EmitTargetCodeForMemcpy(
    SelectionDAG &DAG, const SDLoc &dl, SDValue Chain, SDValue Dst, SDValue Src,
    SDValue Size, Align, Align, bool, bool, MachinePointerInfo,
    MachinePointerInfo) const {
  return emitRamCall(DAG, dl, Chain, "__memcpy_ram", Dst, Src, Size,
                     /*SrcIsAddr=*/true);
}

SDValue I4004SelectionDAGInfo::EmitTargetCodeForMemmove(
    SelectionDAG &DAG, const SDLoc &dl, SDValue Chain, SDValue Dst, SDValue Src,
    SDValue Size, Align, Align, bool, MachinePointerInfo,
    MachinePointerInfo) const {
  return emitRamCall(DAG, dl, Chain, "__memmove_ram", Dst, Src, Size,
                     /*SrcIsAddr=*/true);
}

SDValue I4004SelectionDAGInfo::EmitTargetCodeForMemset(
    SelectionDAG &DAG, const SDLoc &dl, SDValue Chain, SDValue Dst, SDValue Val,
    SDValue Size, Align, bool, bool, MachinePointerInfo) const {
  return emitRamCall(DAG, dl, Chain, "__memset_ram", Dst, Val, Size,
                     /*SrcIsAddr=*/false);
}
