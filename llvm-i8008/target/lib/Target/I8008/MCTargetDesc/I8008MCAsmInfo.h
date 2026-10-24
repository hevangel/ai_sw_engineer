//===-- I8008MCAsmInfo.h - I8008 asm properties ----------------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008MCASMINFO_H
#define LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008MCASMINFO_H

#include "llvm/MC/MCAsmInfoELF.h"

namespace llvm {
class Triple;

class I8008MCAsmInfo : public MCAsmInfoELF {
  void anchor() override;

public:
  explicit I8008MCAsmInfo(const Triple &TT, const MCTargetOptions &Options);

  void printSpecifierExpr(raw_ostream &OS,
                          const MCSpecifierExpr &Expr) const override;
};

} // namespace llvm

#endif // LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008MCASMINFO_H
