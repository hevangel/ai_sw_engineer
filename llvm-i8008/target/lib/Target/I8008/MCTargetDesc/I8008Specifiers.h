//===-- I8008Specifiers.h - I8008 relocation specifiers -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// %lo(expr) / %hi(expr) operand specifiers (spec/asm_syntax.md). Standalone
// (no TableGen includes) so it can sit next to the register-enum .inc.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008SPECIFIERS_H
#define LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008SPECIFIERS_H

#include <cstdint>

namespace llvm {
namespace I8008 {
enum Specifier : uint16_t {
  S_None = 0,
  S_LO,
  S_HI,
};
} // namespace I8008

namespace I8008II {
// MachineOperand target flags: lower a symbol operand as %lo / %hi.
enum TOF : unsigned { MO_NONE = 0, MO_LO = 1, MO_HI = 2 };
} // namespace I8008II
} // namespace llvm

#endif
