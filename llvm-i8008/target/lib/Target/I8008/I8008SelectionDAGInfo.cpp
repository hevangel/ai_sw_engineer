//===-- I8008SelectionDAGInfo.cpp - I8008 SelectionDAG Info --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008SelectionDAGInfo.h"

#define GET_SDNODE_DESC
#include "I8008GenSDNodeInfo.inc"

using namespace llvm;

I8008SelectionDAGInfo::I8008SelectionDAGInfo()
    : SelectionDAGGenTargetInfo(I8008GenSDNodeInfo) {}

I8008SelectionDAGInfo::~I8008SelectionDAGInfo() = default;
