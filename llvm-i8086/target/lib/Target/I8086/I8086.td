//===-- I8086.td - Describe the I8086 Target Machine -------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Top-level TableGen description for the Intel 8086/8088 real-mode target
// (spec/isa.md, spec/abi.md). Only the 8086 instruction set is described; no
// 80186+ instruction exists here.
//
//===----------------------------------------------------------------------===//

include "llvm/Target/Target.td"

include "I8086RegisterInfo.td"
include "I8086InstrFormats.td"
include "I8086InstrInfo.td"

defm : RemapAllTargetPseudoPointerOperands<GR16>;

def : ProcessorModel<"generic", NoSchedModel, []>;
def : ProcessorModel<"i8086", NoSchedModel, []>;
def : ProcessorModel<"i8088", NoSchedModel, []>;

def I8086AsmParser : AsmParser {
  let ShouldEmitMatchRegisterName = 1;
}

def I8086AsmParserVariant : AsmParserVariant {
  int Variant = 0;
  string CommentDelimiter = ";";
  // Register names are bare words in the Intel syntax.
  string RegisterPrefix = "";
}

def I8086InstPrinter : AsmWriter {
  string AsmWriterClassName = "InstPrinter";
  bit isMCAsmWriter = 1;
}

def I8086 : Target {
  let InstructionSet = I8086InstrInfo;
  let AssemblyParsers = [I8086AsmParser];
  let AssemblyParserVariants = [I8086AsmParserVariant];
  let AssemblyWriters = [I8086InstPrinter];
}
