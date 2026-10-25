//===-- I8086MCAsmInfo.h - I8086 asm properties ----------------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086MCASMINFO_H
#define LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086MCASMINFO_H

#include "llvm/MC/MCAsmInfoELF.h"

namespace llvm {
class Triple;

class I8086MCAsmInfo : public MCAsmInfoELF {
  void anchor() override;

public:
  explicit I8086MCAsmInfo(const Triple &TT, const MCTargetOptions &Options);
};

} // namespace llvm

#endif // LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086MCASMINFO_H
