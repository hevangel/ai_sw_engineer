add_llvm_component_group(I4004)

set(LLVM_TARGET_DEFINITIONS I4004.td)

tablegen(LLVM I4004GenAsmMatcher.inc -gen-asm-matcher)
tablegen(LLVM I4004GenAsmWriter.inc -gen-asm-writer)
tablegen(LLVM I4004GenCallingConv.inc -gen-callingconv)
tablegen(LLVM I4004GenDAGISel.inc -gen-dag-isel)
tablegen(LLVM I4004GenInstrInfo.inc -gen-instr-info)
tablegen(LLVM I4004GenMCCodeEmitter.inc -gen-emitter)
tablegen(LLVM I4004GenRegisterInfo.inc -gen-register-info)
tablegen(LLVM I4004GenSDNodeInfo.inc -gen-sd-node-info)
tablegen(LLVM I4004GenSubtargetInfo.inc -gen-subtarget)

add_public_tablegen_target(I4004CommonTableGen)

add_llvm_target(I4004CodeGen
  I4004AsmPrinter.cpp
  I4004ExpandPseudos.cpp
  I4004FrameLowering.cpp
  I4004ISelDAGToDAG.cpp
  I4004ISelLowering.cpp
  I4004InstrInfo.cpp
  I4004MachineFunctionInfo.cpp
  I4004MCInstLower.cpp
  I4004Peephole.cpp
  I4004RegisterInfo.cpp
  I4004SelectionDAGInfo.cpp
  I4004Subtarget.cpp
  I4004TargetMachine.cpp

  LINK_COMPONENTS
  Analysis
  AsmPrinter
  CodeGen
  CodeGenTypes
  Core
  I4004Desc
  I4004Info
  MC
  SelectionDAG
  Support
  Target
  TargetParser
  TransformUtils

  ADD_TO_COMPONENT
  I4004
  )

add_subdirectory(MCTargetDesc)
add_subdirectory(TargetInfo)
add_subdirectory(AsmParser)
add_subdirectory(Disassembler)
