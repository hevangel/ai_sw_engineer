; rep movs and rep stos (see arith.ll for how this file is used).

target triple = "i8086"

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg)
declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg)

; a copy of run-time length: rep movsb
define void @copy(ptr %d, ptr %s, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}

; 64 aligned bytes: rep movsw with CX = 32
define void @copy64(ptr align 2 %d, ptr align 2 %s) {
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %d, ptr align 2 %s, i16 64, i1 false)
  ret void
}

; 33 bytes: odd, so rep movsb
define void @copy33(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 33, i1 false)
  ret void
}

; a small copy is a few loads and stores, not a loop
define void @copy6(ptr align 2 %d, ptr align 2 %s) {
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %d, ptr align 2 %s, i16 6, i1 false)
  ret void
}

; a fill of run-time length: rep stosb
define void @fill(ptr %d, i8 %v, i16 %n) {
  call void @llvm.memset.p0.i16(ptr %d, i8 %v, i16 %n, i1 false)
  ret void
}

; 40 aligned bytes of a variable byte: rep stosw, the byte in both halves of AX
define void @fill40(ptr align 2 %d, i8 %v) {
  call void @llvm.memset.p0.i16(ptr align 2 %d, i8 %v, i16 40, i1 false)
  ret void
}

; 128 aligned zero bytes
define void @zero128(ptr align 2 %d) {
  call void @llvm.memset.p0.i16(ptr align 2 %d, i8 0, i16 128, i1 false)
  ret void
}

; a pattern byte
define void @pattern50(ptr %d) {
  call void @llvm.memset.p0.i16(ptr %d, i8 165, i16 50, i1 false)
  ret void
}
