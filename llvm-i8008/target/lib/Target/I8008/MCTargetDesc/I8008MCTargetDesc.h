//===-- I8008MCTargetDesc.h - I8008 Target Descriptions --------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008MCTARGETDESC_H
#define LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008MCTARGETDESC_H

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

MCCodeEmitter *createI8008MCCodeEmitter(const MCInstrInfo &MCII,
                                        MCContext &Ctx);

MCAsmBackend *createI8008MCAsmBackend(const Target &T,
                                      const MCSubtargetInfo &STI,
                                      const MCRegisterInfo &MRI,
                                      const MCTargetOptions &Options);

std::unique_ptr<MCObjectTargetWriter> createI8008ELFObjectWriter(uint8_t OSABI);

} // namespace llvm

// Relocation specifiers for code-label operand modifiers (abi.md sec.4.4.1).
// In its own header with no TableGen includes so files that already pull the
// register-enum .inc (I8008InstrInfo.h) can use it without a double definition.
#include "I8008Specifiers.h"

// Defines symbolic names for I8008 registers.
#define GET_REGINFO_ENUM
#include "I8008GenRegisterInfo.inc"

// Defines symbolic names for the I8008 instructions.
#define GET_INSTRINFO_ENUM
#define GET_INSTRINFO_MC_HELPER_DECLS
#include "I8008GenInstrInfo.inc"

#define GET_SUBTARGETINFO_ENUM
#include "I8008GenSubtargetInfo.inc"

#endif // LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008MCTARGETDESC_H
