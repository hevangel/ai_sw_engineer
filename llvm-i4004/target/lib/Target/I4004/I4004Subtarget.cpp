//===-- I4004Subtarget.cpp - I4004 Subtarget Information -----------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I4004Subtarget.h"
#include "I4004SelectionDAGInfo.h"
#include "llvm/IR/RuntimeLibcalls.h"
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

void I4004Subtarget::initLibcallLoweringInfo(LibcallLoweringInfo &Info) const {
  // Map the mul/div/rem libcalls to their libgcc-style implementation names
  // (abi.md sec.6 / libi4004rt). Without these, LibCall legalization emits a
  // null symbol name and crashes.
  Info.setLibcallImpl(RTLIB::MUL_I8, RTLIB::impl___mulqi3);
  Info.setLibcallImpl(RTLIB::MUL_I16, RTLIB::impl___mulhi3);
  Info.setLibcallImpl(RTLIB::SDIV_I8, RTLIB::impl___divqi3);
  Info.setLibcallImpl(RTLIB::SDIV_I16, RTLIB::impl___divhi3);
  Info.setLibcallImpl(RTLIB::UDIV_I8, RTLIB::impl___udivqi3);
  Info.setLibcallImpl(RTLIB::UDIV_I16, RTLIB::impl___udivhi3);
  Info.setLibcallImpl(RTLIB::SREM_I8, RTLIB::impl___modqi3);
  Info.setLibcallImpl(RTLIB::SREM_I16, RTLIB::impl___modhi3);
  Info.setLibcallImpl(RTLIB::UREM_I8, RTLIB::impl___umodqi3);
  Info.setLibcallImpl(RTLIB::UREM_I16, RTLIB::impl___umodhi3);
  // memcpy/memset/memmove (llvm.mem* intrinsics -> libi4004rt).
  Info.setLibcallImpl(RTLIB::MEMCPY, RTLIB::impl_memcpy);
  Info.setLibcallImpl(RTLIB::MEMSET, RTLIB::impl_memset);
  Info.setLibcallImpl(RTLIB::MEMMOVE, RTLIB::impl_memmove);
}
