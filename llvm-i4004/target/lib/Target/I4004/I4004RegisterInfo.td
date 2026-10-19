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
  // Sub-register indices for the two 8-bit pairs of a 16-bit double-pair.
  def sub_pair_lo : SubRegIndex<8>;     // low  8 bits = low pair
  def sub_pair_hi : SubRegIndex<8, 8>;  // high 8 bits = high pair
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
// 16-bit double-pairs D0..D3 = {P(2k):P(2k+1)}, for pointers and i16 values
// (abi.md sec.1.3: a 16-bit handle). This mirrors AVR's DREGS: i16 is legal in
// a register-PAIR class while i16 arithmetic is expanded/custom-lowered to i8
// ops on the two sub-pairs. The low pair holds the SRC address byte for AS-0
// load/store; the high pair holds the bank/extended bits.
//===----------------------------------------------------------------------===//

class I4004Double<bits<16> Enc, string n, list<Register> subpairs>
    : RegisterWithSubRegs<n, subpairs> {
  let HWEncoding = Enc;
  let Namespace = "I4004";
  let SubRegIndices = [sub_pair_lo, sub_pair_hi];
}

foreach k = 0-3 in {
  def D#k : I4004Double<k, "d"#k,
                        [!cast<Register>("P"#!mul(k, 2)),
                         !cast<Register>("P"#!add(!mul(k, 2), 1))]>;
}

//===----------------------------------------------------------------------===//
// Register classes.
//===----------------------------------------------------------------------===//

// General 4-bit index registers. (i8 is used as the MVT placeholder; the
// hardware width is 4 bits, encoded 0..15.) In CodeGen these are used as
// physical nibble scratch during post-RA pseudo expansion, not as the class
// for i8 SSA virtuals -- those live in PR8 (abi.md sec.2: "i8 fills the pair").
def GR4 : RegisterClass<"I4004", [i8], 8, (sequence "R%u", 0, 15)>;

// 8-bit register pairs. The natural i8 register class (abi.md sec.2). The
// allocatable pool is P1..P6; P0 (FIN/JIN scratch) and P7 (software SP) are
// reserved in I4004RegisterInfo::getReservedRegs.
def PR8 : RegisterClass<"I4004", [i8], 8, (sequence "P%u", 0, 7)>;

// 16-bit double-pairs: the i16 / pointer class (AVR DREGS analog). D0 contains
// the reserved P0 and D3 contains the reserved P7, so the allocatable i16 pool
// is effectively D1 (P2:P3) and D2 (P4:P5); the allocator still sees all four
// but will not pick a reserved sub-pair.
def DR16 : RegisterClass<"I4004", [i16], 8, (sequence "D%u", 0, 3)>;

// Fixed single-register classes (so TableGen can reference ACC/CY as implicit
// operands and the printer/parser know their class).
def ACCRegs : RegisterClass<"I4004", [i8], 8, (add ACC)> {
  let isAllocatable = 0;
}
def CYRegs : RegisterClass<"I4004", [i8], 8, (add CY)> {
  let isAllocatable = 0;
}
