add_llvm_component_group(I4004)

set(LLVM_TARGET_DEFINITIONS I4004.td)

tablegen(LLVM I4004GenAsmMatcher.inc -gen-asm-matcher)
tablegen(LLVM I4004GenAsmWriter.inc -gen-asm-writer)
tablegen(LLVM I4004GenInstrInfo.inc -gen-instr-info)
tablegen(LLVM I4004GenMCCodeEmitter.inc -gen-emitter)
tablegen(LLVM I4004GenRegisterInfo.inc -gen-register-info)
tablegen(LLVM I4004GenSubtargetInfo.inc -gen-subtarget)

add_public_tablegen_target(I4004CommonTableGen)

# Milestone 2 is the MC layer only (no CodeGen library yet). The target is a
# collection of MC component libraries registered via the experimental-targets
# machinery.
add_subdirectory(MCTargetDesc)
add_subdirectory(TargetInfo)
add_subdirectory(AsmParser)
