//===-- I8008TargetInfo.cpp - I8008 Target Implementation -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "TargetInfo/I8008TargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"
using namespace llvm;

Target &llvm::getTheI8008Target() {
  static Target TheI8008Target;
  return TheI8008Target;
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8008TargetInfo() {
  RegisterTarget<Triple::i8008> X(getTheI8008Target(), "i8008",
                                  "Intel 8008 (MCS-8) [experimental]", "I8008");
}
