//===-- I8086BaseInfo.h - Top level definitions for I8086 MC ---*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The TSFlags layout of I8086InstrFormats.td, shared by the code emitter, the
// decoder and the assembler parser. Keep the two in step.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086BASEINFO_H
#define LLVM_LIB_TARGET_I8086_MCTARGETDESC_I8086BASEINFO_H

#include <cstdint>

namespace llvm {
namespace I8086II {

enum Form : unsigned {
  FormNone = 0,
  FormRegOpc = 1,
  FormSegOpc = 2,
  FormModRM = 3,
  FormDigit = 4,
  FormModRMSeg = 5,
  FormRel8 = 6,
  FormRel16 = 7,
  FormFar = 8,
  FormMoffs = 9,
  FormJccFar = 10,
};

enum ImmKind : unsigned { ImmNone = 0, Imm8 = 1, Imm16 = 2 };

enum Attr : unsigned {
  RegFirst = 1,
  RmReg = 2,
  NoDecode = 4,
  RegOp1 = 8,
  SExtImm = 16,
};

inline unsigned opcode(uint64_t F) { return F & 0xff; }
inline unsigned digit(uint64_t F) { return (F >> 8) & 7; }
inline Form form(uint64_t F) { return static_cast<Form>((F >> 11) & 0xf); }
inline ImmKind imm(uint64_t F) { return static_cast<ImmKind>((F >> 15) & 3); }
inline bool regFirst(uint64_t F) { return (F >> 17) & RegFirst; }
inline bool rmReg(uint64_t F) { return (F >> 17) & RmReg; }
inline bool noDecode(uint64_t F) { return (F >> 17) & NoDecode; }
inline bool regOp1(uint64_t F) { return (F >> 17) & RegOp1; }
inline bool sextImm(uint64_t F) { return (F >> 17) & SExtImm; }
inline bool hasFixed(uint64_t F) { return (F >> 23) & 1; }
inline unsigned fixedByte(uint64_t F) { return (F >> 24) & 0xff; }

/// Does the instruction have a ModRM byte?
inline bool hasModRM(Form F) {
  return F == FormModRM || F == FormDigit || F == FormModRMSeg;
}

} // end namespace I8086II
} // end namespace llvm

#endif
