//===-- I8086SelectionDAGInfo.h - I8086 SelectionDAG Info -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086SELECTIONDAGINFO_H
#define LLVM_LIB_TARGET_I8086_I8086SELECTIONDAGINFO_H

#include "llvm/CodeGen/SelectionDAGTargetInfo.h"

#define GET_SDNODE_ENUM
#include "I8086GenSDNodeInfo.inc"

namespace llvm {

/// memcpy and memset of a size too large to unroll are `rep movs` and
/// `rep stos` (CX = count, DI/SI = addresses; the direction flag is clear).
/// memmove stays a call: its overlap rule needs a direction decision.
class I8086SelectionDAGInfo : public SelectionDAGGenTargetInfo {
public:
  I8086SelectionDAGInfo();
  ~I8086SelectionDAGInfo() override;

  SDValue EmitTargetCodeForMemcpy(SelectionDAG &DAG, const SDLoc &dl,
                                  SDValue Chain, SDValue Dst, SDValue Src,
                                  SDValue Size, Align DstAlign, Align SrcAlign,
                                  bool isVolatile, bool AlwaysInline,
                                  MachinePointerInfo DstPtrInfo,
                                  MachinePointerInfo SrcPtrInfo) const override;

  SDValue EmitTargetCodeForMemset(SelectionDAG &DAG, const SDLoc &dl,
                                  SDValue Chain, SDValue Dst, SDValue Value,
                                  SDValue Size, Align Alignment,
                                  bool isVolatile, bool AlwaysInline,
                                  MachinePointerInfo DstPtrInfo) const override;
};

} // namespace llvm

#endif
