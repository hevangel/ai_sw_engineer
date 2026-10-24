//===-- I8008TargetMachine.h - Define TargetMachine for I8008 --*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Declares the Intel 8008 subclass of TargetMachine.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_I8008TARGETMACHINE_H
#define LLVM_LIB_TARGET_I8008_I8008TARGETMACHINE_H

#include "I8008Subtarget.h"
#include "llvm/CodeGen/CodeGenTargetMachineImpl.h"
#include <optional>

namespace llvm {
class StringRef;

class I8008TargetMachine : public CodeGenTargetMachineImpl {
  std::unique_ptr<TargetLoweringObjectFile> TLOF;
  I8008Subtarget Subtarget;

public:
  I8008TargetMachine(const Target &T, const Triple &TT, StringRef CPU,
                     StringRef FS, const TargetOptions &Options,
                     std::optional<Reloc::Model> RM,
                     std::optional<CodeModel::Model> CM, CodeGenOptLevel OL,
                     bool JIT);
  ~I8008TargetMachine() override;

  const I8008Subtarget *getSubtargetImpl(const Function &F) const override {
    return &Subtarget;
  }

  TargetPassConfig *createPassConfig(PassManagerBase &PM) override;

  /// Interprocedural register allocation: a call keeps the registers its
  /// (already compiled) callee does not touch, instead of assuming every
  /// register is clobbered (abi.md sec.3.3). With four allocatable bytes this
  /// is the difference between spilling around every helper call and not.
  bool useIPRA() const override { return true; }

  TargetLoweringObjectFile *getObjFileLowering() const override {
    return TLOF.get();
  }

  MachineFunctionInfo *
  createMachineFunctionInfo(BumpPtrAllocator &Allocator, const Function &F,
                            const TargetSubtargetInfo *STI) const override;
};

} // end namespace llvm

#endif
