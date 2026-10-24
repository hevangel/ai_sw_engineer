//===-- I8008FixupKinds.h - I8008 Specific Fixup Entries -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008FIXUPKINDS_H
#define LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008FIXUPKINDS_H

#include "llvm/MC/MCFixup.h"

#undef I8008

namespace llvm {
namespace I8008 {

// Same order as the Infos[] array in I8008AsmBackend.cpp.
enum Fixups {
  fixup_16 = FirstTargetFixupKind, // address, 2 bytes little-endian
  fixup_lo8,                       // %lo(expr), 1 byte
  fixup_hi8,                       // %hi(expr), 1 byte
  fixup_8,                         // plain expr in an 8-bit immediate
  LastTargetFixupKind,
  NumTargetFixupKinds = LastTargetFixupKind - FirstTargetFixupKind
};

} // end namespace I8008
} // end namespace llvm

#endif
