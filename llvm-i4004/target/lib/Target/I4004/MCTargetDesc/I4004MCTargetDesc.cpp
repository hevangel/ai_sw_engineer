//===-- I4004MCTargetDesc.cpp - I4004 Target Descriptions ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004MCTargetDesc.h"
#include "I4004InstPrinter.h"
#include "I4004MCAsmInfo.h"
#include "TargetInfo/I4004TargetInfo.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define GET_INSTRINFO_MC_DESC
#define ENABLE_INSTR_PREDICATE_VERIFIER
#include "I4004GenInstrInfo.inc"

#define GET_SUBTARGETINFO_MC_DESC
#include "I4004GenSubtargetInfo.inc"

#define GET_REGINFO_MC_DESC
#include "I4004GenRegisterInfo.inc"

static MCInstrInfo *createI4004MCInstrInfo() {
  MCInstrInfo *X = new MCInstrInfo();
  InitI4004MCInstrInfo(X);
  return X;
}

static MCRegisterInfo *createI4004MCRegisterInfo(const Triple &TT) {
  MCRegisterInfo *X = new MCRegisterInfo();
  // No PC register in the register file (the PC lives in a hidden hardware
  // stack); use ACC as the "return address" placeholder required by the API.
  InitI4004MCRegisterInfo(X, I4004::ACC);
  return X;
}

static MCAsmInfo *createI4004MCAsmInfo(const MCRegisterInfo &MRI,
                                       const Triple &TT,
                                       const MCTargetOptions &Options) {
  return new I4004MCAsmInfo(TT, Options);
}

static MCSubtargetInfo *
createI4004MCSubtargetInfo(const Triple &TT, StringRef CPU, StringRef FS) {
  if (CPU.empty())
    CPU = "generic";
  return createI4004MCSubtargetInfoImpl(TT, CPU, /*TuneCPU=*/CPU, FS);
}

static MCInstPrinter *createI4004MCInstPrinter(const Triple &T,
                                               unsigned SyntaxVariant,
                                               const MCAsmInfo &MAI,
                                               const MCInstrInfo &MII,
                                               const MCRegisterInfo &MRI) {
  if (SyntaxVariant == 0)
    return new I4004InstPrinter(MAI, MII, MRI);
  return nullptr;
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI4004TargetMC() {
  Target &T = getTheI4004Target();
  TargetRegistry::RegisterMCAsmInfo(T, createI4004MCAsmInfo);
  TargetRegistry::RegisterMCInstrInfo(T, createI4004MCInstrInfo);
  TargetRegistry::RegisterMCRegInfo(T, createI4004MCRegisterInfo);
  TargetRegistry::RegisterMCSubtargetInfo(T, createI4004MCSubtargetInfo);
  TargetRegistry::RegisterMCInstPrinter(T, createI4004MCInstPrinter);
  TargetRegistry::RegisterMCCodeEmitter(T, createI4004MCCodeEmitter);
  TargetRegistry::RegisterMCAsmBackend(T, createI4004MCAsmBackend);
}
