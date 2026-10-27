//===-- I8086TargetMachine.cpp - Define TargetMachine for I8086 -----------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086TargetMachine.h"
#include "I8086.h"
#include "I8086MachineFunctionInfo.h"
#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/CodeGen/Passes.h"
#include "llvm/CodeGen/TargetLoweringObjectFileImpl.h"
#include "llvm/CodeGen/TargetPassConfig.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"
#include <optional>

using namespace llvm;

// spec/abi.md D1; rust/i8086-none.json carries the same string.
static StringRef computeDataLayout() {
  return "e-m:e-p:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16";
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void LLVMInitializeI8086Target() {
  RegisterTargetMachine<I8086TargetMachine> X(getTheI8086Target());
  PassRegistry &PR = *PassRegistry::getPassRegistry();
  initializeI8086DAGToDAGISelLegacyPass(PR);
}

I8086TargetMachine::I8086TargetMachine(const Target &T, const Triple &TT,
                                       StringRef CPU, StringRef FS,
                                       const TargetOptions &Options,
                                       std::optional<Reloc::Model> RM,
                                       std::optional<CodeModel::Model> CM,
                                       CodeGenOptLevel OL, bool JIT)
    : CodeGenTargetMachineImpl(T, computeDataLayout(), TT, CPU, FS, Options,
                               RM.value_or(Reloc::Static),
                               getEffectiveCodeModel(CM, CodeModel::Small), OL),
      TLOF(std::make_unique<TargetLoweringObjectFileELF>()),
      Subtarget(TT, std::string(CPU), std::string(FS), *this) {
  // One section per function and data object so the linker can drop what is
  // not used (the whole image has to fit the platform's RAM budget).
  this->Options.FunctionSections = true;
  this->Options.DataSections = true;
  initAsmInfo();
}

I8086TargetMachine::~I8086TargetMachine() = default;

MachineFunctionInfo *I8086TargetMachine::createMachineFunctionInfo(
    BumpPtrAllocator &Allocator, const Function &F,
    const TargetSubtargetInfo *STI) const {
  return I8086MachineFunctionInfo::create<I8086MachineFunctionInfo>(Allocator,
                                                                    F, STI);
}

namespace {
class I8086PassConfig : public TargetPassConfig {
public:
  I8086PassConfig(I8086TargetMachine &TM, PassManagerBase &PM)
      : TargetPassConfig(TM, PM) {}

  bool addInstSelector() override {
    addPass(createI8086ISelDag(getTM<I8086TargetMachine>(), getOptLevel()));
    return false;
  }

  // loop and jcxz, in functions small enough for their 8-bit reach
  void addPreEmitPass() override { addPass(createI8086LoopPeephole()); }
};
} // namespace

TargetPassConfig *I8086TargetMachine::createPassConfig(PassManagerBase &PM) {
  return new I8086PassConfig(*this, PM);
}
