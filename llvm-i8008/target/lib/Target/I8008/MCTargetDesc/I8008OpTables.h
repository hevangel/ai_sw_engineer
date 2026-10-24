//===-- I8008OpTables.h - Opcode lookup by register field ------*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Opcode enums indexed by the DDD/SSS register field (A=0 B=1 C=2 D=3 E=4
// H=5 L=6, 7 = M) and the ALU operation field (AD AC SU SB ND XR OR CP).
// Used by the disassembler and the pseudo expander. Include after the
// generated instruction enum (I8008MCTargetDesc.h).
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008OPTABLES_H
#define LLVM_LIB_TARGET_I8008_MCTARGETDESC_I8008OPTABLES_H

namespace llvm {
namespace I8008Op {

enum Field : unsigned { FA = 0, FB, FC, FD, FE, FH, FL, FM };
enum Alu : unsigned { AD = 0, AC, SU, SB, ND, XR, OR, CP };

// MOV[d][s]: Lds, LdM (s = 7), LMs (d = 7); [7][7] is HLT.
inline constexpr unsigned MOV[8][8] = {
    {I8008::MOV_AA, I8008::MOV_AB, I8008::MOV_AC, I8008::MOV_AD, I8008::MOV_AE, I8008::MOV_AH, I8008::MOV_AL, I8008::MOV_AM},
    {I8008::MOV_BA, I8008::MOV_BB, I8008::MOV_BC, I8008::MOV_BD, I8008::MOV_BE, I8008::MOV_BH, I8008::MOV_BL, I8008::MOV_BM},
    {I8008::MOV_CA, I8008::MOV_CB, I8008::MOV_CC, I8008::MOV_CD, I8008::MOV_CE, I8008::MOV_CH, I8008::MOV_CL, I8008::MOV_CM},
    {I8008::MOV_DA, I8008::MOV_DB, I8008::MOV_DC, I8008::MOV_DD, I8008::MOV_DE, I8008::MOV_DH, I8008::MOV_DL, I8008::MOV_DM},
    {I8008::MOV_EA, I8008::MOV_EB, I8008::MOV_EC, I8008::MOV_ED, I8008::MOV_EE, I8008::MOV_EH, I8008::MOV_EL, I8008::MOV_EM},
    {I8008::MOV_HA, I8008::MOV_HB, I8008::MOV_HC, I8008::MOV_HD, I8008::MOV_HE, I8008::MOV_HH, I8008::MOV_HL, I8008::MOV_HM},
    {I8008::MOV_LA, I8008::MOV_LB, I8008::MOV_LC, I8008::MOV_LD, I8008::MOV_LE, I8008::MOV_LH, I8008::MOV_LL, I8008::MOV_LM},
    {I8008::MOV_MA, I8008::MOV_MB, I8008::MOV_MC, I8008::MOV_MD, I8008::MOV_ME, I8008::MOV_MH, I8008::MOV_ML, I8008::HLT},
};

// MVI[d]: LdI / LMI (d = 7).
inline constexpr unsigned MVI[8] = {I8008::MVI_A, I8008::MVI_B, I8008::MVI_C, I8008::MVI_D, I8008::MVI_E, I8008::MVI_H, I8008::MVI_L, I8008::MVI_M};

// INR/DCR[d], d = 1..6 (0 for A, which has none).
inline constexpr unsigned INR[7] = {0, I8008::INR_B, I8008::INR_C, I8008::INR_D, I8008::INR_E, I8008::INR_H, I8008::INR_L};
inline constexpr unsigned DCR[7] = {0, I8008::DCR_B, I8008::DCR_C, I8008::DCR_D, I8008::DCR_E, I8008::DCR_H, I8008::DCR_L};

// ALU[op][s] (s = 7 is the M form); ALUI[op].
inline constexpr unsigned ALU[8][8] = {
    {I8008::AD_A, I8008::AD_B, I8008::AD_C, I8008::AD_D, I8008::AD_E, I8008::AD_H, I8008::AD_L, I8008::AD_M},
    {I8008::AC_A, I8008::AC_B, I8008::AC_C, I8008::AC_D, I8008::AC_E, I8008::AC_H, I8008::AC_L, I8008::AC_M},
    {I8008::SU_A, I8008::SU_B, I8008::SU_C, I8008::SU_D, I8008::SU_E, I8008::SU_H, I8008::SU_L, I8008::SU_M},
    {I8008::SB_A, I8008::SB_B, I8008::SB_C, I8008::SB_D, I8008::SB_E, I8008::SB_H, I8008::SB_L, I8008::SB_M},
    {I8008::ND_A, I8008::ND_B, I8008::ND_C, I8008::ND_D, I8008::ND_E, I8008::ND_H, I8008::ND_L, I8008::ND_M},
    {I8008::XR_A, I8008::XR_B, I8008::XR_C, I8008::XR_D, I8008::XR_E, I8008::XR_H, I8008::XR_L, I8008::XR_M},
    {I8008::OR_A, I8008::OR_B, I8008::OR_C, I8008::OR_D, I8008::OR_E, I8008::OR_H, I8008::OR_L, I8008::OR_M},
    {I8008::CP_A, I8008::CP_B, I8008::CP_C, I8008::CP_D, I8008::CP_E, I8008::CP_H, I8008::CP_L, I8008::CP_M},
};
inline constexpr unsigned ALUI[8] = {I8008::ADI, I8008::ACI, I8008::SUI, I8008::SBI, I8008::NDI, I8008::XRI, I8008::ORI, I8008::CPI};

// Conditional jumps / calls / returns by (true?, cond): cond C=0 Z=1 S=2 P=3.
inline constexpr unsigned JCC[2][4] = {{I8008::JFC, I8008::JFZ, I8008::JFS, I8008::JFP},
                                       {I8008::JTC, I8008::JTZ, I8008::JTS, I8008::JTP}};
inline constexpr unsigned CCC[2][4] = {{I8008::CFC, I8008::CFZ, I8008::CFS, I8008::CFP},
                                       {I8008::CTC, I8008::CTZ, I8008::CTS, I8008::CTP}};
inline constexpr unsigned RCC[2][4] = {{I8008::RFC, I8008::RFZ, I8008::RFS, I8008::RFP},
                                       {I8008::RTC, I8008::RTZ, I8008::RTS, I8008::RTP}};
enum Cond : unsigned { CondC = 0, CondZ = 1, CondS = 2, CondP = 3 };

} // namespace I8008Op
} // namespace llvm

#endif
