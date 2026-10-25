//===-- I8086FixupKinds.h - I8086 Specific Fixup Entries -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086FIXUPKINDS_H
#define LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086FIXUPKINDS_H

#include "llvm/MC/MCFixup.h"

#undef I8086

namespace llvm {
namespace I8086 {

// Same order as the Infos[] array in I8086AsmBackend.cpp.
enum Fixups {
  fixup_8 = FirstTargetFixupKind, // absolute, 1 byte
  fixup_16,                       // absolute offset, 2 bytes little-endian
  fixup_pc8,                      // PC-relative, 1 byte (short jumps, loops)
  fixup_pc16,                     // PC-relative, 2 bytes (near jump and call)
  LastTargetFixupKind,
  NumTargetFixupKinds = LastTargetFixupKind - FirstTargetFixupKind
};

} // end namespace I8086
} // end namespace llvm

#endif
