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

class I8086SelectionDAGInfo : public SelectionDAGGenTargetInfo {
public:
  I8086SelectionDAGInfo();
  ~I8086SelectionDAGInfo() override;
};

} // namespace llvm

#endif
