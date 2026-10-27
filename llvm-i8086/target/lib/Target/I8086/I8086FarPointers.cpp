//===-- I8086FarPointers.cpp - far data pointers (address space 1) --------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// `ptr addrspace(1)` is a far pointer: 32 bits, the segment in the high word and
// the offset in the low word (the data layout says `p1:32:16:16:16`: the offset
// part is 16 bits and wraps inside the segment, as it does on the chip).
//
// The code generator has no far addressing, so before instruction selection this
// pass takes every use of such a pointer apart:
//
//  - a load or store becomes inline assembly that loads ES with the segment, does
//    the access through ES:[bx] one word (or a trailing byte) at a time, and
//    restores ES (the compiler uses ES only for string instructions and keeps it
//    equal to DS, so ES is saved and restored around the access);
//  - a getelementptr changes the offset word only;
//  - an addrspacecast from a near pointer takes the segment from DS, and one to a
//    near pointer drops the segment;
//  - memcpy and memset with a far operand become a byte loop.
//
// What is left (the pointer as a value: arguments, results, phi, select, compare,
// ptrtoint, inttoptr) is a 32-bit integer to instruction selection.
//
//===----------------------------------------------------------------------===//

#include "I8086.h"
#include "llvm/Analysis/Utils/Local.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/IntrinsicInst.h"
#include "llvm/IR/Module.h"
#include "llvm/Pass.h"
#include "llvm/Support/ErrorHandling.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-far"

namespace {

bool isFar(Type *T) {
  return T->isPointerTy() && T->getPointerAddressSpace() == 1;
}

class FarPointers : public FunctionPass {
public:
  static char ID;
  FarPointers() : FunctionPass(ID) {}
  StringRef getPassName() const override { return "I8086 far pointers"; }

  bool runOnFunction(Function &F) override {
    SmallVector<Instruction *, 16> Work;
    for (BasicBlock &BB : F)
      for (Instruction &I : BB)
        if (needsLowering(I))
          Work.push_back(&I);
    for (Instruction *I : Work)
      lower(*I);
    return !Work.empty();
  }

private:
  static bool needsLowering(const Instruction &I) {
    if (auto *L = dyn_cast<LoadInst>(&I))
      return isFar(L->getPointerOperand()->getType());
    if (auto *S = dyn_cast<StoreInst>(&I))
      return isFar(S->getPointerOperand()->getType());
    if (isa<GetElementPtrInst>(I))
      return isFar(I.getType());
    if (auto *C = dyn_cast<AddrSpaceCastInst>(&I))
      return isFar(C->getSrcTy()) || isFar(C->getDestTy());
    if (auto *M = dyn_cast<MemIntrinsic>(&I)) {
      if (isFar(M->getRawDest()->getType()))
        return true;
      if (auto *T = dyn_cast<MemTransferInst>(M))
        return isFar(T->getRawSource()->getType());
    }
    return false;
  }

  // --- pieces of a far pointer --------------------------------------------
  static Value *seg(IRBuilder<> &B, Value *P) {
    Value *I = B.CreatePtrToInt(P, B.getInt32Ty());
    return B.CreateTrunc(B.CreateLShr(I, 16), B.getInt16Ty());
  }
  static Value *off(IRBuilder<> &B, Value *P) {
    return B.CreateTrunc(B.CreatePtrToInt(P, B.getInt32Ty()), B.getInt16Ty());
  }
  static Value *make(IRBuilder<> &B, Value *Seg, Value *Off) {
    Value *Hi = B.CreateShl(B.CreateZExt(Seg, B.getInt32Ty()), 16);
    Value *V = B.CreateOr(Hi, B.CreateZExt(Off, B.getInt32Ty()));
    return B.CreateIntToPtr(V, PointerType::get(B.getContext(), 1));
  }

  // --- the accesses ---------------------------------------------------------
  static Value *asmLoad(IRBuilder<> &B, unsigned Bytes, Value *Seg, Value *Off) {
    LLVMContext &C = B.getContext();
    Type *Ty = Bytes == 1 ? Type::getInt8Ty(C) : Type::getInt16Ty(C);
    auto *FT = FunctionType::get(Ty, {Type::getInt16Ty(C), Type::getInt16Ty(C)},
                                 false);
    StringRef Mem = Bytes == 1 ? "byte" : "word";
    std::string Text = ("push es\n\tmov es, $1\n\tmov $0, " + Mem +
                        " ptr es:[bx]\n\tpop es")
                           .str();
    InlineAsm *IA = InlineAsm::get(FT, Text, "=&r,r,{bx}", /*sideeffect=*/true);
    return B.CreateCall(IA, {Seg, Off});
  }

  static void asmStore(IRBuilder<> &B, unsigned Bytes, Value *Seg, Value *Off,
                       Value *V) {
    LLVMContext &C = B.getContext();
    Type *Ty = Bytes == 1 ? Type::getInt8Ty(C) : Type::getInt16Ty(C);
    auto *FT = FunctionType::get(
        Type::getVoidTy(C), {Type::getInt16Ty(C), Type::getInt16Ty(C), Ty}, false);
    StringRef Mem = Bytes == 1 ? "byte" : "word";
    std::string Text = ("push es\n\tmov es, $0\n\tmov " + Mem +
                        " ptr es:[bx], $2\n\tpop es")
                           .str();
    InlineAsm *IA = InlineAsm::get(FT, Text, "r,{bx},r", /*sideeffect=*/true);
    B.CreateCall(IA, {Seg, Off, V});
  }

  // The bytes of an integer type or a type that bitcasts to one.
  Type *asInteger(IRBuilder<> &B, Type *T, const DataLayout &DL) {
    if (T->isIntegerTy())
      return T;
    if (T->isPointerTy() || T->isFloatingPointTy())
      return B.getIntNTy(DL.getTypeStoreSizeInBits(T));
    report_fatal_error("I8086: a far load or store of this type is not supported");
  }

  Value *loadValue(IRBuilder<> &B, Type *T, Value *Seg, Value *Off,
                   const DataLayout &DL) {
    Type *IT = asInteger(B, T, DL);
    unsigned Bytes = DL.getTypeStoreSize(IT);
    if (Bytes > 8)
      report_fatal_error("I8086: a far load wider than 64 bits is not supported");
    Type *Wide = B.getIntNTy(Bytes * 8);
    Value *Acc = ConstantInt::get(Wide, 0);
    unsigned At = 0;
    while (At < Bytes) {
      unsigned N = Bytes - At >= 2 ? 2 : 1;
      Value *O = B.CreateAdd(Off, B.getInt16(At));
      Value *Piece = B.CreateZExt(asmLoad(B, N, Seg, O), Wide);
      if (At)
        Piece = B.CreateShl(Piece, At * 8);
      Acc = B.CreateOr(Acc, Piece);
      At += N;
    }
    Value *V = Acc;
    if (IT->getIntegerBitWidth() != Bytes * 8)
      V = B.CreateTrunc(V, IT);
    if (T->isPointerTy())
      return B.CreateIntToPtr(V, T);
    if (T->isFloatingPointTy())
      return B.CreateBitCast(V, T);
    return V;
  }

  void storeValue(IRBuilder<> &B, Value *V, Value *Seg, Value *Off,
                  const DataLayout &DL) {
    Type *T = V->getType();
    Type *IT = asInteger(B, T, DL);
    unsigned Bytes = DL.getTypeStoreSize(IT);
    if (Bytes > 8)
      report_fatal_error("I8086: a far store wider than 64 bits is not supported");
    Type *Wide = B.getIntNTy(Bytes * 8);
    Value *I = V;
    if (T->isPointerTy())
      I = B.CreatePtrToInt(V, IT);
    else if (T->isFloatingPointTy())
      I = B.CreateBitCast(V, IT);
    if (IT->getIntegerBitWidth() != Bytes * 8)
      I = B.CreateZExt(I, Wide);
    unsigned At = 0;
    while (At < Bytes) {
      unsigned N = Bytes - At >= 2 ? 2 : 1;
      Value *Piece = At ? B.CreateLShr(I, At * 8) : I;
      Piece = B.CreateTrunc(Piece, N == 1 ? B.getInt8Ty() : B.getInt16Ty());
      asmStore(B, N, Seg, B.CreateAdd(Off, B.getInt16(At)), Piece);
      At += N;
    }
  }

  // --- the instructions -----------------------------------------------------
  void lower(Instruction &I) {
    const DataLayout &DL = I.getModule()->getDataLayout();
    IRBuilder<> B(&I);
    if (auto *L = dyn_cast<LoadInst>(&I)) {
      Value *P = L->getPointerOperand();
      Value *V = loadValue(B, L->getType(), seg(B, P), off(B, P), DL);
      L->replaceAllUsesWith(V);
    } else if (auto *S = dyn_cast<StoreInst>(&I)) {
      Value *P = S->getPointerOperand();
      storeValue(B, S->getValueOperand(), seg(B, P), off(B, P), DL);
    } else if (auto *G = dyn_cast<GetElementPtrInst>(&I)) {
      Value *P = G->getPointerOperand();
      Value *Delta = emitGEPOffset(&B, DL, G);
      Delta = B.CreateTrunc(B.CreateZExtOrTrunc(Delta, B.getInt32Ty()),
                            B.getInt16Ty());
      G->replaceAllUsesWith(make(B, seg(B, P), B.CreateAdd(off(B, P), Delta)));
    } else if (auto *C = dyn_cast<AddrSpaceCastInst>(&I)) {
      Value *Src = C->getPointerOperand();
      if (isFar(C->getDestTy()) && !isFar(C->getSrcTy())) {
        // the segment of a near pointer is DS
        auto *FT = FunctionType::get(B.getInt16Ty(), false);
        Value *DS = B.CreateCall(InlineAsm::get(FT, "mov $0, ds", "=r", false));
        Value *Off = B.CreatePtrToInt(Src, B.getInt16Ty());
        C->replaceAllUsesWith(make(B, DS, Off));
      } else if (isFar(C->getSrcTy()) && !isFar(C->getDestTy())) {
        C->replaceAllUsesWith(B.CreateIntToPtr(off(B, Src), C->getDestTy()));
      } else {
        report_fatal_error("I8086: unsupported address space cast");
      }
    } else if (auto *M = dyn_cast<MemIntrinsic>(&I)) {
      lowerMem(*M, DL);
      return;
    }
    I.eraseFromParent();
  }

  // The byte at `Base + I`, near or far.
  Value *byteAt(IRBuilder<> &B, Value *Base, Value *Idx, const DataLayout &DL) {
    if (isFar(Base->getType()))
      return loadValue(B, B.getInt8Ty(), seg(B, Base),
                       B.CreateAdd(off(B, Base), Idx), DL);
    return B.CreateLoad(B.getInt8Ty(), B.CreateGEP(B.getInt8Ty(), Base, Idx));
  }
  void storeByteAt(IRBuilder<> &B, Value *Base, Value *Idx, Value *V,
                   const DataLayout &DL) {
    if (isFar(Base->getType()))
      storeValue(B, V, seg(B, Base), B.CreateAdd(off(B, Base), Idx), DL);
    else
      B.CreateStore(V, B.CreateGEP(B.getInt8Ty(), Base, Idx));
  }

  void lowerMem(MemIntrinsic &M, const DataLayout &DL) {
    if (isa<MemMoveInst>(M))
      report_fatal_error("I8086: memmove with a far pointer is not supported");
    // `before` .. loop .. `after`: for (i = 0; i < len; ++i) dst[i] = src[i] / value
    BasicBlock *Pre = M.getParent();
    BasicBlock *After = Pre->splitBasicBlock(&M, "far.mem.after");
    BasicBlock *Loop = BasicBlock::Create(M.getContext(), "far.mem.loop",
                                          Pre->getParent(), After);
    Pre->getTerminator()->eraseFromParent();
    IRBuilder<> B(Pre);
    Type *I16 = B.getInt16Ty();
    Value *Len = B.CreateZExtOrTrunc(M.getLength(), I16);
    B.CreateCondBr(B.CreateICmpEQ(Len, B.getInt16(0)), After, Loop);
    B.SetInsertPoint(Loop);
    PHINode *Idx = B.CreatePHI(I16, 2);
    Idx->addIncoming(B.getInt16(0), Pre);
    Value *V = nullptr;
    if (auto *T = dyn_cast<MemTransferInst>(&M))
      V = byteAt(B, T->getRawSource(), Idx, DL);
    else
      V = cast<MemSetInst>(&M)->getValue();
    storeByteAt(B, M.getRawDest(), Idx, V, DL);
    Value *Next = B.CreateAdd(Idx, B.getInt16(1));
    Idx->addIncoming(Next, Loop);
    B.CreateCondBr(B.CreateICmpULT(Next, Len), Loop, After);
    M.eraseFromParent();
  }
};

} // namespace

char FarPointers::ID = 0;

FunctionPass *llvm::createI8086FarPointers() { return new FarPointers(); }
