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
};

} // namespace llvm

#endif
