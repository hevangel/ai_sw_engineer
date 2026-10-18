//===-- I4004Subtarget.cpp - I4004 Subtarget Information -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004Subtarget.h"
#include "I4004SelectionDAGInfo.h"
#include "llvm/MC/TargetRegistry.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-subtarget"

#define GET_SUBTARGETINFO_TARGET_DESC
#define GET_SUBTARGETINFO_CTOR
#include "I4004GenSubtargetInfo.inc"

void I4004Subtarget::anchor() {}

I4004Subtarget &
I4004Subtarget::initializeSubtargetDependencies(StringRef CPU, StringRef FS) {
  StringRef CPUName = CPU;
  if (CPUName.empty())
    CPUName = "generic";
  ParseSubtargetFeatures(CPUName, /*TuneCPU=*/CPUName, FS);
  return *this;
}

I4004Subtarget::I4004Subtarget(const Triple &TT, const std::string &CPU,
                               const std::string &FS, const TargetMachine &TM)
    : I4004GenSubtargetInfo(TT, CPU, /*TuneCPU=*/CPU, FS),
      InstrInfo(initializeSubtargetDependencies(CPU, FS)), TLInfo(TM, *this),
      FrameLowering(*this) {
  TSInfo = std::make_unique<I4004SelectionDAGInfo>();
}

I4004Subtarget::~I4004Subtarget() = default;

const SelectionDAGTargetInfo *I4004Subtarget::getSelectionDAGInfo() const {
  return TSInfo.get();
}
