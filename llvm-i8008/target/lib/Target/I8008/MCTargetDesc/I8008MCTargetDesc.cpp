//===-- I8008MCTargetDesc.cpp - I8008 Target Descriptions ----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8008MCTargetDesc.h"
#include "I8008InstPrinter.h"
#include "I8008MCAsmInfo.h"
#include "TargetInfo/I8008TargetInfo.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Compiler.h"

using namespace llvm;

#define GET_INSTRINFO_MC_DESC
#define ENABLE_INSTR_PREDICATE_VERIFIER
#include "I8008GenInstrInfo.inc"

#define GET_SUBTARGETINFO_MC_DESC
#include "I8008GenSubtargetInfo.inc"

#define GET_REGINFO_MC_DESC
#include "I8008GenRegisterInfo.inc"

static MCInstrInfo *createI8008MCInstrInfo() {
  MCInstrInfo *X = new MCInstrInfo();
  InitI8008MCInstrInfo(X);
  return X;
}

static MCRegisterInfo *createI8008MCRegisterInfo(const Triple &TT) {
  MCRegisterInfo *X = new MCRegisterInfo();
  // No PC register in the register file (the PC lives in a hidden hardware
  // stack); use A as the "return address" placeholder required by the API.
  InitI8008MCRegisterInfo(X, I8008::A);
  return X;
}

static MCAsmInfo *createI8008MCAsmInfo(const MCRegisterInfo &MRI,
                                       const Triple &TT,
                                       const MCTargetOptions &Options) {
  return new I8008MCAsmInfo(TT, Options);
}

static MCSubtargetInfo *
createI8008MCSubtargetInfo(const Triple &TT, StringRef CPU, StringRef FS) {
  if (CPU.empty())
    CPU = "generic";
  return createI8008MCSubtargetInfoImpl(TT, CPU, /*TuneCPU=*/CPU, FS);
}

static MCInstPrinter *createI8008MCInstPrinter(const Triple &T,
                                               unsigned SyntaxVariant,
                                               const MCAsmInfo &MAI,
                                               const MCInstrInfo &MII,
                                               const MCRegisterInfo &MRI) {
  if (SyntaxVariant == 0)
    return new I8008InstPrinter(MAI, MII, MRI);
  return nullptr;
}

extern "C" LLVM_ABI LLVM_EXTERNAL_VISIBILITY void
LLVMInitializeI8008TargetMC() {
  Target &T = getTheI8008Target();
  TargetRegistry::RegisterMCAsmInfo(T, createI8008MCAsmInfo);
  TargetRegistry::RegisterMCInstrInfo(T, createI8008MCInstrInfo);
  TargetRegistry::RegisterMCRegInfo(T, createI8008MCRegisterInfo);
  TargetRegistry::RegisterMCSubtargetInfo(T, createI8008MCSubtargetInfo);
  TargetRegistry::RegisterMCInstPrinter(T, createI8008MCInstPrinter);
  TargetRegistry::RegisterMCCodeEmitter(T, createI8008MCCodeEmitter);
  TargetRegistry::RegisterMCAsmBackend(T, createI8008MCAsmBackend);
}
