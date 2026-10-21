//===-- I4004Specifiers.h - I4004 relocation specifiers -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Relocation specifiers for code-label operand modifiers (abi.md sec.4.4.1):
//   %lo8(sym)  -> low 8 bits of sym            (R_I4004_LO8)
//   %page(sym) -> page nibble (bits 11..8)     (R_I4004_PAGE4)
// Used by the software-call return token. S_None is a plain immediate.
//
// Standalone (no TableGen .inc includes) so it can be included alongside the
// register-enum .inc without a double definition.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004SPECIFIERS_H
#define LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004SPECIFIERS_H

#include <cstdint>

namespace llvm {
namespace I4004 {
enum Specifier : uint16_t {
  S_None = 0,
  S_LO8,
  S_PAGE,
};
} // namespace I4004
} // namespace llvm

#endif // LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004SPECIFIERS_H
