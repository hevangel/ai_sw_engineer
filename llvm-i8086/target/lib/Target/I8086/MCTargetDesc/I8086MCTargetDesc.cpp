//===-- I8086MCTargetDesc.cpp - I8086 Target Descriptions ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086MCTargetDesc.h"
#include "I8086InstPrinter.h"
#include "I8086MCAsmInfo.h"
#include "TargetInfo/I8086TargetInfo.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define GET_INSTRINFO_MC_DESC
#define ENABLE_INSTR_PREDICATE_VERIFIER
#include "I8086GenInstrInfo.inc"

#define GET_SUBTARGETINFO_MC_DESC
#include "I8086GenSubtargetInfo.inc"

#define GET_REGINFO_MC_DESC
#include "I8086GenRegisterInfo.inc"

static MCInstrInfo *createI8086MCInstrInfo() {
  MCInstrInfo *X = new MCInstrInfo();
  InitI8086MCInstrInfo(X);
  return X;
}

static MCRegisterInfo *createI8086MCRegisterInfo(const Triple &TT) {
  MCRegisterInfo *X = new MCRegisterInfo();
  // The return address lives on the stack, not in a register; IP is not in the
  // register file, so SP stands in for the "return address register".
  InitI8086MCRegisterInfo(X, I8086::SP);
  return X;
}

static MCAsmInfo *createI8086MCAsmInfo(const MCRegisterInfo &MRI,
                                       const Triple &TT,
                                       const MCTargetOptions &Options) {
  return new I8086MCAsmInfo(TT, Options);
}

static MCSubtargetInfo *createI8086MCSubtargetInfo(const Triple &TT,
                                                   StringRef CPU,
                                                   StringRef FS) {
  if (CPU.empty())
    CPU = "generic";
  return createI8086MCSubtargetInfoImpl(TT, CPU, /*TuneCPU=*/CPU, FS);
}

static MCInstPrinter *createI8086MCInstPrinter(const Triple &T,
                                               unsigned SyntaxVariant,
                                               const MCAsmInfo &MAI,
                                               const MCInstrInfo &MII,
                                               const MCRegisterInfo &MRI) {
  if (SyntaxVariant == 0)
    return new I8086InstPrinter(MAI, MII, MRI);
  return nullptr;
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8086TargetMC() {
  Target &T = getTheI8086Target();
  TargetRegistry::RegisterMCAsmInfo(T, createI8086MCAsmInfo);
  TargetRegistry::RegisterMCInstrInfo(T, createI8086MCInstrInfo);
  TargetRegistry::RegisterMCRegInfo(T, createI8086MCRegisterInfo);
  TargetRegistry::RegisterMCSubtargetInfo(T, createI8086MCSubtargetInfo);
  TargetRegistry::RegisterMCInstPrinter(T, createI8086MCInstPrinter);
  TargetRegistry::RegisterMCCodeEmitter(T, createI8086MCCodeEmitter);
  TargetRegistry::RegisterMCAsmBackend(T, createI8086MCAsmBackend);
}
