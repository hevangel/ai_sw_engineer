//===-- I4004SelectionDAGInfo.h - I4004 SelectionDAG Info ------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004SELECTIONDAGINFO_H
#define LLVM_LIB_TARGET_I4004_I4004SELECTIONDAGINFO_H

#include "llvm/CodeGen/SelectionDAGTargetInfo.h"

#define GET_SDNODE_ENUM
#include "I4004GenSDNodeInfo.inc"

namespace llvm {

class I4004SelectionDAGInfo : public SelectionDAGGenTargetInfo {
public:
  I4004SelectionDAGInfo();
  ~I4004SelectionDAGInfo() override;

  // Lower llvm.memcpy/memset/memmove to the libi4004rt RAM helpers
  // (__memcpy_ram/__memset_ram/__memmove_ram) instead of the standard C-ABI
  // memcpy/memset. The RAM helpers take SRC address bytes in P1/P2 and a NIBBLE
  // count in P3 (abi.md sec.6: a byte occupies two 4002 characters, so the
  // nibble count is 2x the byte Size). This also keeps the third argument in a
  // register (P3) rather than spilling a standard memcpy's size arg to the
  // outgoing-arg RAM area.
  SDValue EmitTargetCodeForMemcpy(SelectionDAG &DAG, const SDLoc &dl,
                                  SDValue Chain, SDValue Dst, SDValue Src,
                                  SDValue Size, Align DstAlign, Align SrcAlign,
                                  bool isVolatile, bool AlwaysInline,
                                  MachinePointerInfo DstPtrInfo,
                                  MachinePointerInfo SrcPtrInfo) const override;

  SDValue EmitTargetCodeForMemmove(SelectionDAG &DAG, const SDLoc &dl,
                                   SDValue Chain, SDValue Dst, SDValue Src,
                                   SDValue Size, Align DstAlign, Align SrcAlign,
                                   bool isVolatile,
                                   MachinePointerInfo DstPtrInfo,
                                   MachinePointerInfo SrcPtrInfo) const override;

  SDValue EmitTargetCodeForMemset(SelectionDAG &DAG, const SDLoc &dl,
                                  SDValue Chain, SDValue Dst, SDValue Val,
                                  SDValue Size, Align Alignment,
                                  bool isVolatile, bool AlwaysInline,
                                  MachinePointerInfo DstPtrInfo) const override;
};

} // namespace llvm

#endif
