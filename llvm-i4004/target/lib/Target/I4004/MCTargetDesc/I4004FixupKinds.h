//===-- I4004FixupKinds.h - I4004 Specific Fixup Entries -------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004FIXUPKINDS_H
#define LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004FIXUPKINDS_H

#include "llvm/MC/MCFixup.h"

#undef I4004

namespace llvm {
namespace I4004 {

// This table must be in the same order as the MCFixupKindInfo Infos[] array in
// I4004AsmBackend.cpp.
enum Fixups {
  // 12-bit absolute code address (JUN/JMS): nibble A3 in byte 0 low nibble,
  // A2A1 in byte 1.
  fixup_addr12 = FirstTargetFixupKind,
  // 8-bit page-local code address (JCN/ISZ) in byte 1.
  fixup_page8,
  // 8-bit immediate (FIM) in byte 1.
  fixup_imm8,
  // 8-bit page-local FIN index in byte 1.
  fixup_fin8,
  // Low 8 bits of a (code) symbol -> FIM second word (byte 1). Used by the
  // software-call sequence to materialise a return label's page-local offset
  // (abi.md sec.4.4.1 / sec.4.4.2). From %lo8(sym).
  fixup_lo8,
  // Page nibble (bits 11..8) of a (code) symbol -> FIM second word (byte 1),
  // in the LOW nibble of that byte. Used by the software-call sequence to
  // materialise a return label's page. From %page(sym).
  fixup_page4,

  // Marker
  LastTargetFixupKind,
  NumTargetFixupKinds = LastTargetFixupKind - FirstTargetFixupKind
};

} // end namespace I4004
} // end namespace llvm

#endif // LLVM_LIB_TARGET_I4004_MCTARGETDESC_I4004FIXUPKINDS_H
