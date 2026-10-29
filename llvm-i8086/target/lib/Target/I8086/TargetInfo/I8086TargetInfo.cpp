//===-- I8086TargetInfo.cpp - I8086 Target Implementation -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"
using namespace llvm;

Target &llvm::getTheI8086Target() {
  static Target TheI8086Target;
  return TheI8086Target;
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8086TargetInfo() {
  RegisterTarget<Triple::i8086> X(getTheI8086Target(), "i8086",
                                  "Intel 8086/8088 (real mode) [experimental]",
                                  "I8086");
}
