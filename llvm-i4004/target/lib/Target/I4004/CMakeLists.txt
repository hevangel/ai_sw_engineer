add_llvm_component_group(I4004)

set(LLVM_TARGET_DEFINITIONS I4004.td)

tablegen(LLVM I4004GenAsmMatcher.inc -gen-asm-matcher)
tablegen(LLVM I4004GenAsmWriter.inc -gen-asm-writer)
tablegen(LLVM I4004GenInstrInfo.inc -gen-instr-info)
tablegen(LLVM I4004GenMCCodeEmitter.inc -gen-emitter)
tablegen(LLVM I4004GenRegisterInfo.inc -gen-register-info)
tablegen(LLVM I4004GenSubtargetInfo.inc -gen-subtarget)

add_public_tablegen_target(I4004CommonTableGen)

# Milestone 2 is the MC layer only. We still register a minimal CodeGen
# component library (LLVMI4004CodeGen) so the target resolves as a proper
# component for tools that link "all-targets"; it contains only the init hook
# (no TargetMachine yet - that is milestone 3).
add_llvm_target(I4004CodeGen
  I4004TargetMachine.cpp

  LINK_COMPONENTS
  I4004Desc
  I4004Info
  MC
  Support
  Target

  ADD_TO_COMPONENT
  I4004
  )

add_subdirectory(MCTargetDesc)
add_subdirectory(TargetInfo)
add_subdirectory(AsmParser)
add_subdirectory(Disassembler)
