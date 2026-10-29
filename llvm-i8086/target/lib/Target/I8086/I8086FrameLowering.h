//===-- I8086FrameLowering.h - Define frame lowering for I8086 -*- C++ -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// spec/abi.md D4. The stack grows down. SP cannot address memory, so a
// function with stack objects keeps a frame pointer in BP:
//
//   push bp ; mov bp, sp ; push bx/si/di (those used) ; sub sp, N
//   ... [bp - 2k - ...] locals, [bp + 4 + ...] stack arguments ...
//   lea sp, [bp - 2k] ; pop di/si/bx ; pop bp ; ret
//
// A function without stack objects has no frame pointer and only pushes the
// callee-saved registers it uses. Stack pointer stores below SP are never
// emitted (an interrupt would overwrite them), so callee-saved registers are
// pushed and locals are allocated before use.
//
//===----------------------------------------------------------------------===//

#ifndef LLVM_LIB_TARGET_I8086_I8086FRAMELOWERING_H
#define LLVM_LIB_TARGET_I8086_I8086FRAMELOWERING_H

#include "llvm/CodeGen/TargetFrameLowering.h"

namespace llvm {

class I8086Subtarget;

class I8086FrameLowering : public TargetFrameLowering {
protected:
  bool hasFPImpl(const MachineFunction &MF) const override;

public:
  explicit I8086FrameLowering(const I8086Subtarget &STI);

  void emitPrologue(MachineFunction &MF, MachineBasicBlock &MBB) const override;
  void emitEpilogue(MachineFunction &MF, MachineBasicBlock &MBB) const override;

  void determineCalleeSaves(MachineFunction &MF, BitVector &SavedRegs,
                            RegScavenger *RS = nullptr) const override;
  bool assignCalleeSavedSpillSlots(
      MachineFunction &MF, const TargetRegisterInfo *TRI,
      std::vector<CalleeSavedInfo> &CSI) const override;
  bool spillCalleeSavedRegisters(MachineBasicBlock &MBB,
                                 MachineBasicBlock::iterator MI,
                                 ArrayRef<CalleeSavedInfo> CSI,
                                 const TargetRegisterInfo *TRI) const override;
  bool restoreCalleeSavedRegisters(
      MachineBasicBlock &MBB, MachineBasicBlock::iterator MI,
      MutableArrayRef<CalleeSavedInfo> CSI,
      const TargetRegisterInfo *TRI) const override;

  bool hasReservedCallFrame(const MachineFunction &MF) const override;

  MachineBasicBlock::iterator
  eliminateCallFramePseudoInstr(MachineFunction &MF, MachineBasicBlock &MBB,
                                MachineBasicBlock::iterator I) const override;
};

} // end namespace llvm

#endif
