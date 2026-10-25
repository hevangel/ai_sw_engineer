//===-- I8008TargetTransformInfo.h - I8008 specific TTI ---------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Only the CodeGenPrepare hook is target-specific (spec/codegen.md sec.2):
// `(x as u16) << 8` used by an `or`/`add` in another block is sunk next to
// it, so instruction selection sees a byte pair (two register moves) instead
// of a 16-bit OR of a value it knows nothing about.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008TARGETTRANSFORMINFO_H
#define LLVM_LIB_TARGET_I8008_I8008TARGETTRANSFORMINFO_H

#include "I8008TargetMachine.h"
#include "llvm/CodeGen/BasicTTIImpl.h"

namespace llvm {

class I8008TTIImpl final : public BasicTTIImplBase<I8008TTIImpl> {
  using BaseT = BasicTTIImplBase<I8008TTIImpl>;
  friend BaseT;

  const I8008Subtarget *ST;
  const I8008TargetLowering *TLI;

  const I8008Subtarget *getST() const { return ST; }
  const I8008TargetLowering *getTLI() const { return TLI; }

public:
  explicit I8008TTIImpl(const I8008TargetMachine *TM, const Function &F)
      : BaseT(TM, F.getDataLayout()), ST(TM->getSubtargetImpl(F)),
        TLI(ST->getTargetLowering()) {}

  bool isProfitableToSinkOperands(Instruction *I,
                                  SmallVectorImpl<Use *> &Ops) const override;
};

} // namespace llvm

#endif
