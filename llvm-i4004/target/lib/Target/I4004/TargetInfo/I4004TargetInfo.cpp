//===-- I4004TargetInfo.cpp - I4004 Target Implementation -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"
using namespace llvm;

Target &llvm::getTheI4004Target() {
  static Target TheI4004Target;
  return TheI4004Target;
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI4004TargetInfo() {
  RegisterTarget<Triple::i4004> X(getTheI4004Target(), "i4004",
                                  "Intel 4004 (MCS-4) [experimental]", "I4004");
}
