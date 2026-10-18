//===-- I4004.td - Describe the I4004 Target Machine -------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Top-level TableGen description for the Intel 4004 (MCS-4) target.
// Milestone 2 is the MC layer only: registers, instruction encodings, the asm
// parser/printer and the MC code emitter. No instruction selection yet.
//
//===----------------------------------------------------------------------===//

include "llvm/Target/Target.td"

//===----------------------------------------------------------------------===//
// Subtarget features (none yet; a single generic 4004 model).
//===----------------------------------------------------------------------===//

//===----------------------------------------------------------------------===//
// Register File, Instruction Descriptions
//===----------------------------------------------------------------------===//

include "I4004RegisterInfo.td"
include "I4004InstrInfo.td"

//===----------------------------------------------------------------------===//
// I4004 processor definition
//===----------------------------------------------------------------------===//

def : ProcessorModel<"generic", NoSchedModel, []>;
def : ProcessorModel<"i4004", NoSchedModel, []>;

//===----------------------------------------------------------------------===//
// Assembly parser / printer
//===----------------------------------------------------------------------===//

def I4004AsmParser : AsmParser {
  let ShouldEmitMatchRegisterName = 1;
}

def I4004AsmParserVariant : AsmParserVariant {
  int Variant = 0;
  // Recognize both ';' (MCS-4) and the usual comment forms; the lexer config
  // in the MCAsmInfo handles the actual comment characters.
  string CommentDelimiter = ";";
}

def I4004InstPrinter : AsmWriter {
  string AsmWriterClassName = "InstPrinter";
  bit isMCAsmWriter = 1;
}

//===----------------------------------------------------------------------===//
// Target declaration
//===----------------------------------------------------------------------===//

def I4004 : Target {
  let InstructionSet = I4004InstrInfo;
  let AssemblyParsers = [I4004AsmParser];
  let AssemblyParserVariants = [I4004AsmParserVariant];
  let AssemblyWriters = [I4004InstPrinter];
}
