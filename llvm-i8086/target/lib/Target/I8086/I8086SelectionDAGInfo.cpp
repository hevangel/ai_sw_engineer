//===-- I8086SelectionDAGInfo.cpp - I8086 SelectionDAG Info ---------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086SelectionDAGInfo.h"

#define GET_SDNODE_DESC
#include "I8086GenSDNodeInfo.inc"

using namespace llvm;

I8086SelectionDAGInfo::I8086SelectionDAGInfo()
    : SelectionDAGGenTargetInfo(I8086GenSDNodeInfo) {}

I8086SelectionDAGInfo::~I8086SelectionDAGInfo() = default;
