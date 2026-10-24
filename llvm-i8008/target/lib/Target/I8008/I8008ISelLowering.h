//===-- I8008ISelLowering.h - I8008 DAG Lowering Interface -----*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008ISELLOWERING_H
#define LLVM_LIB_TARGET_I8008_I8008ISELLOWERING_H

#include "llvm/CodeGen/TargetLowering.h"

namespace llvm {

class I8008Subtarget;

// Condition codes carried by the BRCC / SELECT_CC immediate.
namespace I8008CC {
enum CondCode : int {
  EQ = 0,
  NE = 1,
  ULT = 2,
  UGE = 3,
  UGT = 4,
  ULE = 5,
  SLT = 6,
  SGE = 7,
  SGT = 8,
  SLE = 9,
};
inline int invert(int CC) {
  switch (CC) {
  case EQ: return NE;
  case NE: return EQ;
  case ULT: return UGE;
  case UGE: return ULT;
  case UGT: return ULE;
  case ULE: return UGT;
  case SLT: return SGE;
  case SGE: return SLT;
  case SGT: return SLE;
  default: return SGT; // SLE
  }
}
/// The code that holds when the operands are swapped (a < b == b > a).
inline int swapped(int CC) {
  switch (CC) {
  case ULT: return UGT;
  case UGT: return ULT;
  case UGE: return ULE;
  case ULE: return UGE;
  case SLT: return SGT;
  case SGT: return SLT;
  case SGE: return SLE;
  case SLE: return SGE;
  default: return CC; // EQ, NE
  }
}
} // namespace I8008CC

class I8008TargetLowering : public TargetLowering {
public:
  explicit I8008TargetLowering(const TargetMachine &TM,
                               const I8008Subtarget &STI);

  const char *getTargetNodeName(unsigned Opcode) const override;
  SDValue LowerOperation(SDValue Op, SelectionDAG &DAG) const override;

  SDValue LowerFormalArguments(SDValue Chain, CallingConv::ID CallConv,
                               bool IsVarArg,
                               const SmallVectorImpl<ISD::InputArg> &Ins,
                               const SDLoc &DL, SelectionDAG &DAG,
                               SmallVectorImpl<SDValue> &InVals) const override;
  bool CanLowerReturn(CallingConv::ID CallConv, MachineFunction &MF,
                      bool IsVarArg,
                      const SmallVectorImpl<ISD::OutputArg> &Outs,
                      LLVMContext &Context, const Type *RetTy) const override;
  SDValue LowerReturn(SDValue Chain, CallingConv::ID CallConv, bool IsVarArg,
                      const SmallVectorImpl<ISD::OutputArg> &Outs,
                      const SmallVectorImpl<SDValue> &OutVals, const SDLoc &DL,
                      SelectionDAG &DAG) const override;
  SDValue LowerCall(TargetLowering::CallLoweringInfo &CLI,
                    SmallVectorImpl<SDValue> &InVals) const override;

  EVT getSetCCResultType(const DataLayout &, LLVMContext &,
                         EVT VT) const override {
    return MVT::i8;
  }
  MVT getScalarShiftAmountTy(const DataLayout &, EVT) const override {
    return MVT::i8;
  }
  EVT getTypeForExtReturn(LLVMContext &, EVT VT,
                          ISD::NodeType) const override {
    return VT.bitsLT(MVT::i8) ? EVT(MVT::i8) : VT;
  }

  MachineBasicBlock *
  EmitInstrWithCustomInserter(MachineInstr &MI,
                              MachineBasicBlock *MBB) const override;

private:
  const I8008Subtarget &Subtarget;

  SDValue LowerCallResult(SDValue Chain, SDValue InGlue,
                          CallingConv::ID CallConv, bool IsVarArg,
                          const SmallVectorImpl<ISD::InputArg> &Ins,
                          const SDLoc &DL, SelectionDAG &DAG,
                          SmallVectorImpl<SDValue> &InVals) const;
  SDValue LowerAddress(SDValue Op, SelectionDAG &DAG) const;
  SDValue LowerBR_CC(SDValue Op, SelectionDAG &DAG) const;
  SDValue LowerSELECT_CC(SDValue Op, SelectionDAG &DAG) const;
  SDValue LowerShift(SDValue Op, SelectionDAG &DAG) const;
  /// Address of byte Off of the memory-argument area __args_<Fn>.
  SDValue argArea(SelectionDAG &DAG, const SDLoc &DL, StringRef Fn,
                  int64_t Off) const;
};

} // end namespace llvm

#endif
