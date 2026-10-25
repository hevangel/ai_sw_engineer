//===-- I8008TargetTransformInfo.cpp - I8008 specific TTI -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008TargetTransformInfo.h"
#include "llvm/IR/PatternMatch.h"

using namespace llvm;
using namespace llvm::PatternMatch;

bool I8008TTIImpl::isProfitableToSinkOperands(Instruction *I,
                                              SmallVectorImpl<Use *> &Ops) const {
  if (!I->getType()->isIntegerTy(16) ||
      (I->getOpcode() != Instruction::Or && I->getOpcode() != Instruction::Add))
    return false;
  bool Any = false;
  for (unsigned Op = 0; Op < 2; ++Op) {
    auto *Shl = dyn_cast<Instruction>(I->getOperand(Op));
    Value *X;
    if (!Shl || !match(Shl, m_Shl(m_ZExt(m_Value(X)), m_SpecificInt(8))) ||
        !X->getType()->isIntegerTy(8))
      continue;
    // Sink the zext with the shift, so both sit next to I.
    Ops.push_back(&Shl->getOperandUse(0));
    Ops.push_back(&I->getOperandUse(Op));
    Any = true;
  }
  return Any;
}
