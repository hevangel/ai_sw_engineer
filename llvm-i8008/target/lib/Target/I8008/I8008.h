//===-- I8008.h - Top-level interface for I8008 ----------------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008_H
#define LLVM_LIB_TARGET_I8008_I8008_H

#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/PassRegistry.h"
#include "llvm/Support/CodeGen.h"

namespace llvm {
class I8008TargetMachine;
class FunctionPass;

FunctionPass *createI8008ISelDag(I8008TargetMachine &TM,
                                 CodeGenOptLevel OptLevel);
// Post-RA: pseudos -> real 8008 code (spec/codegen.md sec.3).
FunctionPass *createI8008ExpandPseudosPass();
// After expansion: drop redundant H:L loads (spec/codegen.md sec.5).
FunctionPass *createI8008HLTrackPass();

void initializeI8008DAGToDAGISelLegacyPass(PassRegistry &);
void initializeI8008ExpandPseudosPass(PassRegistry &);
void initializeI8008HLTrackPass(PassRegistry &);

} // namespace llvm

#endif
