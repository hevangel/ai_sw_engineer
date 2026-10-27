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
  // Soft floating point and 128-bit integers: the compiler-rt names (provided by
  // Rust's compiler_builtins, rt/README.md).
  Info.setLibcallImpl(RTLIB::ADD_F32, RTLIB::impl___addsf3);
  Info.setLibcallImpl(RTLIB::ADD_F64, RTLIB::impl___adddf3);
  Info.setLibcallImpl(RTLIB::DIV_F32, RTLIB::impl___divsf3);
  Info.setLibcallImpl(RTLIB::DIV_F64, RTLIB::impl___divdf3);
  Info.setLibcallImpl(RTLIB::FPEXT_F32_F64, RTLIB::impl___extendsfdf2);
  Info.setLibcallImpl(RTLIB::FPROUND_F64_F32, RTLIB::impl___truncdfsf2);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F32_I128, RTLIB::impl___fixsfti);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F32_I32, RTLIB::impl___fixsfsi);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F32_I64, RTLIB::impl___fixsfdi);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F64_I128, RTLIB::impl___fixdfti);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F64_I32, RTLIB::impl___fixdfsi);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F64_I64, RTLIB::impl___fixdfdi);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F32_I128, RTLIB::impl___fixunssfti);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F32_I32, RTLIB::impl___fixunssfsi);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F32_I64, RTLIB::impl___fixunssfdi);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F64_I128, RTLIB::impl___fixunsdfti);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F64_I32, RTLIB::impl___fixunsdfsi);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F64_I64, RTLIB::impl___fixunsdfdi);
  Info.setLibcallImpl(RTLIB::MUL_F32, RTLIB::impl___mulsf3);
  Info.setLibcallImpl(RTLIB::MUL_F64, RTLIB::impl___muldf3);
  Info.setLibcallImpl(RTLIB::MUL_I128, RTLIB::impl___multi3);
  Info.setLibcallImpl(RTLIB::OEQ_F32, RTLIB::impl___eqsf2);
  Info.setLibcallImpl(RTLIB::OEQ_F64, RTLIB::impl___eqdf2);
  Info.setLibcallImpl(RTLIB::OGE_F32, RTLIB::impl___gesf2);
  Info.setLibcallImpl(RTLIB::OGE_F64, RTLIB::impl___gedf2);
  Info.setLibcallImpl(RTLIB::OGT_F32, RTLIB::impl___gtsf2);
  Info.setLibcallImpl(RTLIB::OGT_F64, RTLIB::impl___gtdf2);
  Info.setLibcallImpl(RTLIB::OLE_F32, RTLIB::impl___lesf2);
  Info.setLibcallImpl(RTLIB::OLE_F64, RTLIB::impl___ledf2);
  Info.setLibcallImpl(RTLIB::OLT_F32, RTLIB::impl___ltsf2);
  Info.setLibcallImpl(RTLIB::OLT_F64, RTLIB::impl___ltdf2);
  Info.setLibcallImpl(RTLIB::SDIV_I128, RTLIB::impl___divti3);
  Info.setLibcallImpl(RTLIB::SHL_I128, RTLIB::impl___ashlti3);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I128_F32, RTLIB::impl___floattisf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I128_F64, RTLIB::impl___floattidf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I32_F32, RTLIB::impl___floatsisf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I32_F64, RTLIB::impl___floatsidf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I64_F32, RTLIB::impl___floatdisf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I64_F64, RTLIB::impl___floatdidf);
  Info.setLibcallImpl(RTLIB::SRA_I128, RTLIB::impl___ashrti3);
  Info.setLibcallImpl(RTLIB::SREM_I128, RTLIB::impl___modti3);
  Info.setLibcallImpl(RTLIB::SRL_I128, RTLIB::impl___lshrti3);
  Info.setLibcallImpl(RTLIB::SUB_F32, RTLIB::impl___subsf3);
  Info.setLibcallImpl(RTLIB::SUB_F64, RTLIB::impl___subdf3);
  Info.setLibcallImpl(RTLIB::UDIV_I128, RTLIB::impl___udivti3);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I128_F32, RTLIB::impl___floatuntisf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I128_F64, RTLIB::impl___floatuntidf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I32_F32, RTLIB::impl___floatunsisf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I32_F64, RTLIB::impl___floatunsidf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I64_F32, RTLIB::impl___floatundisf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I64_F64, RTLIB::impl___floatundidf);
  Info.setLibcallImpl(RTLIB::UNE_F32, RTLIB::impl___nesf2);
  Info.setLibcallImpl(RTLIB::UNE_F64, RTLIB::impl___nedf2);
  Info.setLibcallImpl(RTLIB::UO_F32, RTLIB::impl___unordsf2);
  Info.setLibcallImpl(RTLIB::UO_F64, RTLIB::impl___unorddf2);
  Info.setLibcallImpl(RTLIB::UREM_I128, RTLIB::impl___umodti3);
  Info.setLibcallImpl(RTLIB::FPEXT_F16_F32, RTLIB::impl___extendhfsf2);
  Info.setLibcallImpl(RTLIB::FPEXT_F16_F64, RTLIB::impl___extendhfdf2);
  Info.setLibcallImpl(RTLIB::FPROUND_F32_F16, RTLIB::impl___truncsfhf2);
  Info.setLibcallImpl(RTLIB::FPROUND_F64_F16, RTLIB::impl___truncdfhf2);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F16_I128, RTLIB::impl___fixhfti);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F16_I32, RTLIB::impl___fixhfsi);
  Info.setLibcallImpl(RTLIB::FPTOSINT_F16_I64, RTLIB::impl___fixhfdi);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F16_I128, RTLIB::impl___fixunshfti);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F16_I32, RTLIB::impl___fixunshfsi);
  Info.setLibcallImpl(RTLIB::FPTOUINT_F16_I64, RTLIB::impl___fixunshfdi);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I128_F16, RTLIB::impl___floattihf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I32_F16, RTLIB::impl___floatsihf);
  Info.setLibcallImpl(RTLIB::SINTTOFP_I64_F16, RTLIB::impl___floatdihf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I128_F16, RTLIB::impl___floatuntihf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I32_F16, RTLIB::impl___floatunsihf);
  Info.setLibcallImpl(RTLIB::UINTTOFP_I64_F16, RTLIB::impl___floatundihf);
  Info.setLibcallImpl(RTLIB::MULO_I32, RTLIB::impl___mulosi4);
  Info.setLibcallImpl(RTLIB::MULO_I64, RTLIB::impl___mulodi4);
  Info.setLibcallImpl(RTLIB::MULO_I128, RTLIB::impl___muloti4);
  Info.setLibcallImpl(RTLIB::MEMCPY, RTLIB::impl_memcpy);
  Info.setLibcallImpl(RTLIB::MEMSET, RTLIB::impl_memset);
  Info.setLibcallImpl(RTLIB::MEMMOVE, RTLIB::impl_memmove);
}
