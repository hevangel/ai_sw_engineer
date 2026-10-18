//===-- I4004RegisterInfo.td - I4004 Register defs ---------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Intel 4004 registers (spec/abi.md sec.sec.2).
//
//  * ACC  - 4-bit accumulator, fixed hardware resource.
//  * CY   - 1-bit carry, fixed.
//  * R0..R15 - sixteen 4-bit index registers (class GR4). Encoding = index.
//  * P0..P7  - eight register pairs {R(2n):R(2n+1)} (class PR8). The pair's
//              encoding is n (0..7); the instruction word carries it in the
//              RRR field with bit0 selecting FIM/SRC/FIN/JIN sub-op.
//
// ASSUMPTION (abi.md sec.sec.1.2): i4 is a legal register VALUE width but never an
// addressable memory unit. The MC layer only needs the register *encodings*;
// value-type legality is a CodeGen concern (milestone 3), so here GR4 just
// carries i8 as a convenient MVT placeholder for the 4-bit registers.
//
//===----------------------------------------------------------------------===//

class I4004Reg<bits<16> Enc, string n, list<string> alt = []> : Register<n> {
  let HWEncoding = Enc;
  let Namespace = "I4004";
  let AltNames = alt;
}

// Pair register: an even/odd index-register pair.
class I4004RegPair<bits<16> Enc, string n, list<Register> subregs,
                   list<string> alt = []>
    : RegisterWithSubRegs<n, subregs> {
  let HWEncoding = Enc;
  let Namespace = "I4004";
  let AltNames = alt;
}

//===----------------------------------------------------------------------===//
// Fixed accumulator and carry.
//===----------------------------------------------------------------------===//

def ACC : I4004Reg<0, "acc">;
def CY  : I4004Reg<0, "cy">;

//===----------------------------------------------------------------------===//
// 16 index registers R0..R15. HWEncoding = register index (RRRR field).
//===----------------------------------------------------------------------===//

foreach i = 0-15 in {
  def R#i : I4004Reg<i, "r"#i>;
}

//===----------------------------------------------------------------------===//
// Sub-register indices for the two nibbles of a pair.
//===----------------------------------------------------------------------===//

let Namespace = "I4004" in {
  def sub_hi : SubRegIndex<4>;      // even register R(2n), high nibble
  def sub_lo : SubRegIndex<4, 4>;   // odd register  R(2n+1), low nibble
}

//===----------------------------------------------------------------------===//
// 8 register pairs P0..P7. HWEncoding = pair index n (RRR field, 0..7).
// Alias R0R1 .. R14R15 accepted by the asm parser (asm_syntax.md).
//===----------------------------------------------------------------------===//

foreach n = 0-7 in {
  def P#n : I4004RegPair<n, "p"#n,
                         [!cast<Register>("R"#!mul(n, 2)),
                          !cast<Register>("R"#!add(!mul(n, 2), 1))],
                         ["r"#!mul(n,2)#"r"#!add(!mul(n,2),1)]> {
    let SubRegIndices = [sub_hi, sub_lo];
  }
}

//===----------------------------------------------------------------------===//
// Register classes.
//===----------------------------------------------------------------------===//

// General 4-bit index registers. (i8 is used as the MVT placeholder; the
// hardware width is 4 bits, encoded 0..15.)
def GR4 : RegisterClass<"I4004", [i8], 8, (sequence "R%u", 0, 15)>;

// 8-bit register pairs.
def PR8 : RegisterClass<"I4004", [i16], 8, (sequence "P%u", 0, 7)>;

// Fixed single-register classes (so TableGen can reference ACC/CY as implicit
// operands and the printer/parser know their class).
def ACCRegs : RegisterClass<"I4004", [i8], 8, (add ACC)> {
  let isAllocatable = 0;
}
def CYRegs : RegisterClass<"I4004", [i8], 8, (add CY)> {
  let isAllocatable = 0;
}
