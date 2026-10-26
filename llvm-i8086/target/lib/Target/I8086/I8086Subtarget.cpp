//===-- I8086Subtarget.cpp - I8086 Subtarget Information ------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#include "I8086Subtarget.h"
#include "I8086SelectionDAGInfo.h"
#include "llvm/IR/RuntimeLibcalls.h"
#include "llvm/MC/TargetRegistry.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-subtarget"

#define GET_SUBTARGETINFO_TARGET_DESC
#define GET_SUBTARGETINFO_CTOR
#include "I8086GenSubtargetInfo.inc"

void I8086Subtarget::anchor() {}

I8086Subtarget &
I8086Subtarget::initializeSubtargetDependencies(StringRef CPU, StringRef FS) {
  StringRef CPUName = CPU;
  if (CPUName.empty())
    CPUName = "generic";
  ParseSubtargetFeatures(CPUName, /*TuneCPU=*/CPUName, FS);
  return *this;
}

I8086Subtarget::I8086Subtarget(const Triple &TT, const std::string &CPU,
                               const std::string &FS, const TargetMachine &TM)
    : I8086GenSubtargetInfo(TT, CPU, /*TuneCPU=*/CPU, FS),
      InstrInfo(initializeSubtargetDependencies(CPU, FS)), TLInfo(TM, *this),
      FrameLowering(*this) {
  TSInfo = std::make_unique<I8086SelectionDAGInfo>();
}

I8086Subtarget::~I8086Subtarget() = default;

const SelectionDAGTargetInfo *I8086Subtarget::getSelectionDAGInfo() const {
  return TSInfo.get();
}

void I8086Subtarget::initLibcallLoweringInfo(LibcallLoweringInfo &Info) const {
  // libgcc-style names of the runtime helpers (abi.md, Runtime library).
  // Without these, libcall legalization emits a null symbol name.
  Info.setLibcallImpl(RTLIB::MUL_I16, RTLIB::impl___mulhi3);
  Info.setLibcallImpl(RTLIB::SDIV_I16, RTLIB::impl___divhi3);
  Info.setLibcallImpl(RTLIB::UDIV_I16, RTLIB::impl___udivhi3);
  Info.setLibcallImpl(RTLIB::SREM_I16, RTLIB::impl___modhi3);
  Info.setLibcallImpl(RTLIB::UREM_I16, RTLIB::impl___umodhi3);
  Info.setLibcallImpl(RTLIB::MUL_I32, RTLIB::impl___mulsi3);
  Info.setLibcallImpl(RTLIB::SDIV_I32, RTLIB::impl___divsi3);
  Info.setLibcallImpl(RTLIB::UDIV_I32, RTLIB::impl___udivsi3);
  Info.setLibcallImpl(RTLIB::SREM_I32, RTLIB::impl___modsi3);
  Info.setLibcallImpl(RTLIB::UREM_I32, RTLIB::impl___umodsi3);
  Info.setLibcallImpl(RTLIB::SHL_I32, RTLIB::impl___ashlsi3);
  Info.setLibcallImpl(RTLIB::SRL_I32, RTLIB::impl___lshrsi3);
  Info.setLibcallImpl(RTLIB::SRA_I32, RTLIB::impl___ashrsi3);
  Info.setLibcallImpl(RTLIB::MUL_I64, RTLIB::impl___muldi3);
  Info.setLibcallImpl(RTLIB::SDIV_I64, RTLIB::impl___divdi3);
  Info.setLibcallImpl(RTLIB::UDIV_I64, RTLIB::impl___udivdi3);
  Info.setLibcallImpl(RTLIB::SREM_I64, RTLIB::impl___moddi3);
  Info.setLibcallImpl(RTLIB::UREM_I64, RTLIB::impl___umoddi3);
  Info.setLibcallImpl(RTLIB::SHL_I64, RTLIB::impl___ashldi3);
  Info.setLibcallImpl(RTLIB::SRL_I64, RTLIB::impl___lshrdi3);
  Info.setLibcallImpl(RTLIB::SRA_I64, RTLIB::impl___ashrdi3);
  Info.setLibcallImpl(RTLIB::MEMCPY, RTLIB::impl_memcpy);
  Info.setLibcallImpl(RTLIB::MEMSET, RTLIB::impl_memset);
  Info.setLibcallImpl(RTLIB::MEMMOVE, RTLIB::impl_memmove);
}
