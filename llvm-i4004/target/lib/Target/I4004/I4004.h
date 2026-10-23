//===-- I4004.h - Top-level interface for I4004 ----------------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Top-level interface for the Intel 4004 (MCS-4) CodeGen backend. Milestone 3b:
// the instruction-selection backbone that makes `llc -mtriple=i4004` compile
// real code per spec/abi.md.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_I4004_H
#define LLVM_LIB_TARGET_I4004_I4004_H

#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/PassRegistry.h"
#include "llvm/Support/CodeGen.h"

namespace llvm {
class I4004TargetMachine;
class FunctionPass;
class PassRegistry;

// Create the ISel DAG-to-DAG pass (legacy PM).
FunctionPass *createI4004ISelDag(I4004TargetMachine &TM,
                                 CodeGenOptLevel OptLevel);

// Post-RA pseudo expander (expands the ACC-centric pseudos into real 4004
// instruction sequences).
FunctionPass *createI4004ExpandPseudosPass();
/// `-i4004-softcall` or `-i4004-banked-ram` is on (I4004ExpandPseudos.cpp).
bool i4004LegacyLayout();

// Post-expansion peephole: removes redundant FIM/SRC pointer setups (the
// largest single source of code size on this target).
FunctionPass *createI4004PeepholePass();

// Far-branch relaxation: makes JCN/ISZ page-independent (JCN !cc,.L; JUN Dest)
// so a function may span 256-byte ROM pages.
FunctionPass *createI4004BranchRelaxPass();

// Pre-RA: compare-with-constant branches (BRCC* with a MOVi operand -> *ri).
FunctionPass *createI4004ImmComparePass();

void initializeI4004DAGToDAGISelLegacyPass(PassRegistry &);
void initializeI4004ExpandPseudosPass(PassRegistry &);
void initializeI4004PeepholePass(PassRegistry &);
void initializeI4004BranchRelaxPass(PassRegistry &);
void initializeI4004ImmComparePass(PassRegistry &);

} // namespace llvm

#endif // LLVM_LIB_TARGET_I4004_I4004_H
