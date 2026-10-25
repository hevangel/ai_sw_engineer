//===-- I8086Target.cpp - I8086 target entry point ------------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The target has an assembler, a disassembler and an object writer; code
// generation (TargetMachine, instruction selection) is the next milestone
// (plans/implementation_plan.md). This keeps the registration symbol that
// LLVM's target lists expect.
//
//===----------------------------------------------------------------------===//

#include "llvm/Support/Compiler.h"

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void LLVMInitializeI8086Target() {}
