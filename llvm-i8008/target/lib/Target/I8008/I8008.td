//===-- I8008.td - Describe the I8008 Target Machine -------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Top-level TableGen description for the Intel 8008 target (spec/codegen.md).
//
//===----------------------------------------------------------------------===//

include "llvm/Target/Target.td"

include "I8008RegisterInfo.td"
include "I8008InstrInfo.td"
include "I8008CallingConv.td"
include "I8008InstrCodeGen.td"

defm : RemapAllTargetPseudoPointerOperands<GR16>;

def : ProcessorModel<"generic", NoSchedModel, []>;
def : ProcessorModel<"i8008", NoSchedModel, []>;

def I8008AsmParser : AsmParser {
  let ShouldEmitMatchRegisterName = 0;
}

def I8008AsmParserVariant : AsmParserVariant {
  int Variant = 0;
  string CommentDelimiter = ";";
}

def I8008InstPrinter : AsmWriter {
  string AsmWriterClassName = "InstPrinter";
  bit isMCAsmWriter = 1;
}

def I8008 : Target {
  let InstructionSet = I8008InstrInfo;
  let AssemblyParsers = [I8008AsmParser];
  let AssemblyParserVariants = [I8008AsmParserVariant];
  let AssemblyWriters = [I8008InstPrinter];
}
