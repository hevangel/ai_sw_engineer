//===-- I4004TargetMachine.cpp - Define TargetMachine for I4004 ----------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Milestone 2 is the MC layer only. There is no code generator yet, so this
// file provides just the component's init hook (LLVMInitializeI4004Target),
// which InitializeAllTargets() calls from CodeGen tools. It deliberately
// registers no TargetMachine: llc/codegen support is milestone 3. llvm-mc only
// needs TargetInfo + TargetMC + AsmParser, which live in the other libraries.
//
//===----------------------------------------------------------------------===//

#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void LLVMInitializeI4004Target() {
  // No TargetMachine registered yet (milestone 3). Referencing the target here
  // keeps the symbol and the TargetInfo library linked for codegen tools that
  // call InitializeAllTargets().
  (void)getTheI4004Target();
}
