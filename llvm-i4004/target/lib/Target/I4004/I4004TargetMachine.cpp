//===-- I4004TargetMachine.cpp - Define TargetMachine for I4004 ----------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Top-level implementation for the Intel 4004 target (milestone 3b: CodeGen
// backbone). Registers the TargetMachine so `llc -mtriple=i4004` can lower IR.
//
//===----------------------------------------------------------------------===//

#include "I4004TargetMachine.h"
#include "I4004.h"
#include "I4004MachineFunctionInfo.h"
#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/CodeGen/Passes.h"
#include "llvm/CodeGen/TargetLoweringObjectFileImpl.h"
#include "llvm/CodeGen/TargetPassConfig.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"
#include <optional>

using namespace llvm;

// The data layout is the authoritative abi.md sec.1 string. We set it
// explicitly rather than via Triple::computeDataLayout so the ABI document is
// the single source of truth (rust/i4004-none.json must match it char-for-char;
// CI greps both -- plans/testplan.md).
static StringRef computeDataLayout() {
  return "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8";
}

static Reloc::Model getEffectiveRelocModel(std::optional<Reloc::Model> RM) {
  return RM.value_or(Reloc::Static);
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void LLVMInitializeI4004Target() {
  RegisterTargetMachine<I4004TargetMachine> X(getTheI4004Target());
  PassRegistry &PR = *PassRegistry::getPassRegistry();
  initializeI4004DAGToDAGISelLegacyPass(PR);
  initializeI4004ExpandPseudosPass(PR);
  initializeI4004PeepholePass(PR);
  initializeI4004BranchRelaxPass(PR);
}

I4004TargetMachine::I4004TargetMachine(const Target &T, const Triple &TT,
                                       StringRef CPU, StringRef FS,
                                       const TargetOptions &Options,
                                       std::optional<Reloc::Model> RM,
                                       std::optional<CodeModel::Model> CM,
                                       CodeGenOptLevel OL, bool JIT)
    : CodeGenTargetMachineImpl(T, computeDataLayout(), TT, CPU, FS, Options,
                               getEffectiveRelocModel(RM),
                               getEffectiveCodeModel(CM, CodeModel::Small), OL),
      TLOF(std::make_unique<TargetLoweringObjectFileELF>()),
      Subtarget(TT, std::string(CPU), std::string(FS), *this) {
  initAsmInfo();
}

I4004TargetMachine::~I4004TargetMachine() = default;

MachineFunctionInfo *I4004TargetMachine::createMachineFunctionInfo(
    BumpPtrAllocator &Allocator, const Function &F,
    const TargetSubtargetInfo *STI) const {
  return I4004MachineFunctionInfo::create<I4004MachineFunctionInfo>(Allocator, F,
                                                                    STI);
}

namespace {
class I4004PassConfig : public TargetPassConfig {
public:
  I4004PassConfig(I4004TargetMachine &TM, PassManagerBase &PM)
      : TargetPassConfig(TM, PM) {}

  I4004TargetMachine &getI4004TargetMachine() const {
    return getTM<I4004TargetMachine>();
  }

  bool addInstSelector() override;
  void addPreEmitPass() override;
};
} // namespace

TargetPassConfig *I4004TargetMachine::createPassConfig(PassManagerBase &PM) {
  return new I4004PassConfig(*this, PM);
}

bool I4004PassConfig::addInstSelector() {
  addPass(createI4004ISelDag(getI4004TargetMachine(), getOptLevel()));
  return false;
}

void I4004PassConfig::addPreEmitPass() {
  // Expand the ACC-centric pseudos into real MCS-4 instruction sequences.
  addPass(createI4004ExpandPseudosPass());
  // Make conditional branches page-independent so functions may span pages.
  addPass(createI4004BranchRelaxPass());
  // Then elide redundant FIM/SRC pointer setups the expansion leaves behind.
  addPass(createI4004PeepholePass());
}
