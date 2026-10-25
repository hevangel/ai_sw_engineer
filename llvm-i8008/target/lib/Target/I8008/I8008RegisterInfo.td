//===-- I8008RegisterInfo.td - I8008 Register defs ---------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Intel 8008 registers (spec/abi.md sec.2). HWEncoding is the DDD/SSS field
// of the MCS-8 manual: A=0 B=1 C=2 D=3 E=4 H=5 L=6 (M=7 is memory, not a
// register).
//
//  * GR8  = B, C, D, E      allocatable i8
//  * GR16 = BC, DE          allocatable i16 / pointers (high byte first name)
//  * A                      accumulator, reserved scratch for expansions
//  * H, L (HL)              memory pointer, reserved scratch for expansions
//  * F                      the four flags, reserved
//
//===----------------------------------------------------------------------===//

class I8008Reg<bits<16> Enc, string n> : Register<n> {
  let HWEncoding = Enc;
  let Namespace = "I8008";
}

let Namespace = "I8008" in {
  def sub_lo : SubRegIndex<8>;
  def sub_hi : SubRegIndex<8, 8>;
}

def A : I8008Reg<0, "a">;
def B : I8008Reg<1, "b">;
def C : I8008Reg<2, "c">;
def D : I8008Reg<3, "d">;
def E : I8008Reg<4, "e">;
def H : I8008Reg<5, "h">;
def L : I8008Reg<6, "l">;
def F : I8008Reg<0, "f">;

class I8008Pair<bits<16> Enc, string n, Register Lo, Register Hi>
    : RegisterWithSubRegs<n, [Lo, Hi]> {
  let HWEncoding = Enc;
  let Namespace = "I8008";
  let SubRegIndices = [sub_lo, sub_hi];
  let CoveredBySubRegs = 1;
}

def BC : I8008Pair<1, "bc", C, B>;
def DE : I8008Pair<3, "de", E, D>;
def HL : I8008Pair<5, "hl", L, H>;

def GR8  : RegisterClass<"I8008", [i8], 8, (add B, C, D, E)>;
def GR16 : RegisterClass<"I8008", [i16], 8, (add BC, DE)>;

// Fixed registers, referenced as implicit operands only.
def ACCR : RegisterClass<"I8008", [i8], 8, (add A)> { let isAllocatable = 0; }
def HLR8 : RegisterClass<"I8008", [i8], 8, (add H, L)> { let isAllocatable = 0; }
def HLR  : RegisterClass<"I8008", [i16], 8, (add HL)> { let isAllocatable = 0; }
def FLAGR : RegisterClass<"I8008", [i8], 8, (add F)> { let isAllocatable = 0; }
