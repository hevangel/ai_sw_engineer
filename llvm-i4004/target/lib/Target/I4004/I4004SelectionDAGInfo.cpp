//===-- I4004SelectionDAGInfo.cpp - I4004 SelectionDAG Info --------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004SelectionDAGInfo.h"

#define GET_SDNODE_DESC
#include "I4004GenSDNodeInfo.inc"

using namespace llvm;

I4004SelectionDAGInfo::I4004SelectionDAGInfo()
    : SelectionDAGGenTargetInfo(I4004GenSDNodeInfo) {}

I4004SelectionDAGInfo::~I4004SelectionDAGInfo() = default;
