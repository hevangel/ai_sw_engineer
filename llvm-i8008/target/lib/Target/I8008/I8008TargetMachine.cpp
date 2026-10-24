//===-- I8008TargetMachine.cpp - Define TargetMachine for I8008 ----------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008TargetMachine.h"
#include "I8008.h"
#include "I8008MachineFunctionInfo.h"
#include "TargetInfo/I8008TargetInfo.h"
#include "llvm/CodeGen/Passes.h"
#include "llvm/CodeGen/TargetLoweringObjectFileImpl.h"
#include "llvm/CodeGen/TargetPassConfig.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/CommandLine.h"
#include "llvm/Support/Compiler.h"
#include <optional>

using namespace llvm;

// abi.md sec.1; rust/i8008-none.json carries the same string.
static StringRef computeDataLayout() {
  return "e-m:e-p:16:8-i8:8-i16:8-i32:8-i64:8-n8-a:8";
}

static cl::opt<bool> NoHLTrack("i8008-no-hltrack",
                               cl::desc("I8008: keep every H:L load"),
                               cl::init(false));

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void LLVMInitializeI8008Target() {
  RegisterTargetMachine<I8008TargetMachine> X(getTheI8008Target());
  PassRegistry &PR = *PassRegistry::getPassRegistry();
  initializeI8008DAGToDAGISelLegacyPass(PR);
  initializeI8008ExpandPseudosPass(PR);
  initializeI8008HLTrackPass(PR);
}

I8008TargetMachine::I8008TargetMachine(const Target &T, const Triple &TT,
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
  initAsmInfo();
}

I8008TargetMachine::~I8008TargetMachine() = default;

MachineFunctionInfo *I8008TargetMachine::createMachineFunctionInfo(
    BumpPtrAllocator &Allocator, const Function &F,
    const TargetSubtargetInfo *STI) const {
  return I8008MachineFunctionInfo::create<I8008MachineFunctionInfo>(Allocator, F,
                                                                    STI);
}

namespace {
class I8008PassConfig : public TargetPassConfig {
public:
  I8008PassConfig(I8008TargetMachine &TM, PassManagerBase &PM)
      : TargetPassConfig(TM, PM) {}

  bool addInstSelector() override {
    addPass(createI8008ISelDag(getTM<I8008TargetMachine>(), getOptLevel()));
    return false;
  }
  void addPreEmitPass() override {
    addPass(createI8008ExpandPseudosPass());
    if (!NoHLTrack)
      addPass(createI8008HLTrackPass());
  }
};
} // namespace

TargetPassConfig *I8008TargetMachine::createPassConfig(PassManagerBase &PM) {
  return new I8008PassConfig(*this, PM);
}
