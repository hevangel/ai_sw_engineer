//===-- I4004MCTargetDesc.h - I4004 Target Descriptions --------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004MCTARGETDESC_H
#define LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004MCTARGETDESC_H

#include "llvm/Support/DataTypes.h"
#include <memory>

namespace llvm {
class Target;
class MCAsmBackend;
class MCCodeEmitter;
class MCInstrInfo;
class MCSubtargetInfo;
class MCRegisterInfo;
class MCContext;
class MCTargetOptions;
class MCObjectTargetWriter;

MCCodeEmitter *createI4004MCCodeEmitter(const MCInstrInfo &MCII,
                                        MCContext &Ctx);

MCAsmBackend *createI4004MCAsmBackend(const Target &T,
                                      const MCSubtargetInfo &STI,
                                      const MCRegisterInfo &MRI,
                                      const MCTargetOptions &Options);

std::unique_ptr<MCObjectTargetWriter> createI4004ELFObjectWriter(uint8_t OSABI);

} // namespace llvm

// Defines symbolic names for I4004 registers.
#define GET_REGINFO_ENUM
#include "I4004GenRegisterInfo.inc"

// Defines symbolic names for the I4004 instructions.
#define GET_INSTRINFO_ENUM
#define GET_INSTRINFO_MC_HELPER_DECLS
#include "I4004GenInstrInfo.inc"

#define GET_SUBTARGETINFO_ENUM
#include "I4004GenSubtargetInfo.inc"

#endif // LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004MCTARGETDESC_H
