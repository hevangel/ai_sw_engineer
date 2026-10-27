//===-- I8086.h - Top-level interface for I8086 -----------------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086_H
#define LLVM_LIB_TARGET_I8086_I8086_H

#include "llvm/PassRegistry.h"
#include "llvm/Support/CodeGen.h"

namespace llvm {
class I8086TargetMachine;
class FunctionPass;

FunctionPass *createI8086ISelDag(I8086TargetMachine &TM,
                                 CodeGenOptLevel OptLevel);

FunctionPass *createI8086LoopPeephole();
FunctionPass *createI8086FarPointers();

void initializeI8086DAGToDAGISelLegacyPass(PassRegistry &);

} // namespace llvm

#endif
