//===-- I8086SelectionDAGInfo.cpp - I8086 SelectionDAG Info ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086SelectionDAGInfo.h"
#include "I8086RegisterInfo.h"
#include "llvm/CodeGen/SelectionDAG.h"

#define GET_SDNODE_DESC
#include "I8086GenSDNodeInfo.inc"

using namespace llvm;

I8086SelectionDAGInfo::I8086SelectionDAGInfo()
    : SelectionDAGGenTargetInfo(I8086GenSDNodeInfo) {}

I8086SelectionDAGInfo::~I8086SelectionDAGInfo() = default;

SDValue I8086SelectionDAGInfo::EmitTargetCodeForMemcpy(
    SelectionDAG &DAG, const SDLoc &dl, SDValue Chain, SDValue Dst, SDValue Src,
    SDValue Size, Align DstAlign, Align SrcAlign, bool isVolatile,
    bool AlwaysInline, MachinePointerInfo DstPtrInfo,
    MachinePointerInfo SrcPtrInfo) const {
  auto *C = dyn_cast<ConstantSDNode>(Size);
  if (C && C->isZero())
    return Chain;
  // Words when the size is a known even number and both ends are aligned.
  bool Words = C && C->getZExtValue() % 2 == 0 && DstAlign >= Align(2) &&
               SrcAlign >= Align(2);
  SDValue Count =
      Words ? DAG.getConstant(C->getZExtValue() / 2, dl, MVT::i16) : Size;

  SDValue Glue;
  Chain = DAG.getCopyToReg(Chain, dl, I8086::CX, Count, Glue);
  Glue = Chain.getValue(1);
  Chain = DAG.getCopyToReg(Chain, dl, I8086::DI, Dst, Glue);
  Glue = Chain.getValue(1);
  Chain = DAG.getCopyToReg(Chain, dl, I8086::SI, Src, Glue);
  Glue = Chain.getValue(1);
  SDVTList Tys = DAG.getVTList(MVT::Other, MVT::Glue);
  SDValue Ops[] = {Chain, Glue};
  return DAG.getNode(Words ? I8086ISD::REP_MOVSW : I8086ISD::REP_MOVSB, dl, Tys,
                     Ops);
}

SDValue I8086SelectionDAGInfo::EmitTargetCodeForMemset(
    SelectionDAG &DAG, const SDLoc &dl, SDValue Chain, SDValue Dst,
    SDValue Value, SDValue Size, Align Alignment, bool isVolatile,
    bool AlwaysInline, MachinePointerInfo DstPtrInfo) const {
  auto *C = dyn_cast<ConstantSDNode>(Size);
  if (C && C->isZero())
    return Chain;
  bool Words = C && C->getZExtValue() % 2 == 0 && Alignment >= Align(2);
  SDValue Count =
      Words ? DAG.getConstant(C->getZExtValue() / 2, dl, MVT::i16) : Size;

  SDValue Fill;
  if (Words) {
    // The byte in both halves of AX.
    if (auto *VC = dyn_cast<ConstantSDNode>(Value)) {
      uint64_t B = VC->getZExtValue() & 0xFF;
      Fill = DAG.getConstant(B * 0x0101, dl, MVT::i16);
    } else {
      SDValue Z = DAG.getZExtOrTrunc(Value, dl, MVT::i16);
      Fill = DAG.getNode(ISD::OR, dl, MVT::i16, Z,
                         DAG.getNode(ISD::SHL, dl, MVT::i16, Z,
                                     DAG.getShiftAmountConstant(8, MVT::i16, dl)));
    }
  } else {
    Fill = DAG.getZExtOrTrunc(Value, dl, MVT::i8);
  }

  SDValue Glue;
  Chain = DAG.getCopyToReg(Chain, dl, I8086::CX, Count, Glue);
  Glue = Chain.getValue(1);
  Chain = DAG.getCopyToReg(Chain, dl, I8086::DI, Dst, Glue);
  Glue = Chain.getValue(1);
  Chain = DAG.getCopyToReg(Chain, dl, Words ? I8086::AX : I8086::AL, Fill, Glue);
  Glue = Chain.getValue(1);
  SDVTList Tys = DAG.getVTList(MVT::Other, MVT::Glue);
  SDValue Ops[] = {Chain, Glue};
  return DAG.getNode(Words ? I8086ISD::REP_STOSW : I8086ISD::REP_STOSB, dl, Tys,
                     Ops);
}
