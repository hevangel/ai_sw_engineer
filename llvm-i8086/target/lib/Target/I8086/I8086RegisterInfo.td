//===-- I8086RegisterInfo.td - I8086 Register defs ---------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Registers in encoding order (the ModRM reg/rm and opcode+r fields): the 16-bit
// AX CX DX BX SP BP SI DI, their byte halves AL CL DL BL AH CH DH BH, and the
// segment registers ES CS SS DS. The encoding number is the hardware field value.
//
//===----------------------------------------------------------------------===//

let Namespace = "I8086" in {
  def sub_8bit    : SubRegIndex<8>;
  def sub_8bit_hi : SubRegIndex<8, 8>;
}

class I8086Reg<string n, bits<4> enc> : Register<n> {
  let Namespace = "I8086";
  let HWEncoding{3-0} = enc;
}

class I8086Reg16<string n, bits<4> enc, list<Register> subs>
    : RegisterWithSubRegs<n, subs> {
  let Namespace = "I8086";
  let HWEncoding{3-0} = enc;
  let SubRegIndices = [sub_8bit, sub_8bit_hi];
  let CoveredBySubRegs = 1;
}

// Byte registers: AL CL DL BL are the low halves of AX CX DX BX; AH CH DH BH the high.
def AL : I8086Reg<"al", 0>;
def CL : I8086Reg<"cl", 1>;
def DL : I8086Reg<"dl", 2>;
def BL : I8086Reg<"bl", 3>;
def AH : I8086Reg<"ah", 4>;
def CH : I8086Reg<"ch", 5>;
def DH : I8086Reg<"dh", 6>;
def BH : I8086Reg<"bh", 7>;

def AX : I8086Reg16<"ax", 0, [AL, AH]>;
def CX : I8086Reg16<"cx", 1, [CL, CH]>;
def DX : I8086Reg16<"dx", 2, [DL, DH]>;
def BX : I8086Reg16<"bx", 3, [BL, BH]>;
def SP : I8086Reg<"sp", 4>;
def BP : I8086Reg<"bp", 5>;
def SI : I8086Reg<"si", 6>;
def DI : I8086Reg<"di", 7>;

def ES : I8086Reg<"es", 0>;
def CS : I8086Reg<"cs", 1>;
def SS : I8086Reg<"ss", 2>;
def DS : I8086Reg<"ds", 3>;

def FLAGS : I8086Reg<"flags", 0>;

def GR8  : RegisterClass<"I8086", [i8], 8, (add AL, CL, DL, BL, AH, CH, DH, BH)>;
def GR16 : RegisterClass<"I8086", [i16], 16, (add AX, CX, DX, BX, SP, BP, SI, DI)>;
def SEGREG : RegisterClass<"I8086", [i16], 16, (add ES, CS, SS, DS)>;
def SEGREGNC : RegisterClass<"I8086", [i16], 16, (add ES, SS, DS)>;
def FLAGSREG : RegisterClass<"I8086", [i16], 16, (add FLAGS)>;

// Registers an instruction names literally.
def GR8AL  : RegisterClass<"I8086", [i8], 8, (add AL)>;
def GR8CL  : RegisterClass<"I8086", [i8], 8, (add CL)>;
def GR16AX : RegisterClass<"I8086", [i16], 16, (add AX)>;
def GR16DX : RegisterClass<"I8086", [i16], 16, (add DX)>;
