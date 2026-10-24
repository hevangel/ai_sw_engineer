//===-- I8008SelectionDAGInfo.h - I8008 SelectionDAG Info ------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008SELECTIONDAGINFO_H
#define LLVM_LIB_TARGET_I8008_I8008SELECTIONDAGINFO_H

#include "llvm/CodeGen/SelectionDAGTargetInfo.h"

#define GET_SDNODE_ENUM
#include "I8008GenSDNodeInfo.inc"

namespace llvm {

// memcpy/memmove/memset use the generic lowering: the standard C routines in
// rt/mem.s with the count as a memory argument (abi.md sec.6).
class I8008SelectionDAGInfo : public SelectionDAGGenTargetInfo {
public:
  I8008SelectionDAGInfo();
  ~I8008SelectionDAGInfo() override;
};

} // namespace llvm

#endif
