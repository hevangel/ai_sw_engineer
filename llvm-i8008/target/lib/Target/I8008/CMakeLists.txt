add_llvm_component_group(I8008)

set(LLVM_TARGET_DEFINITIONS I8008.td)

tablegen(LLVM I8008GenAsmMatcher.inc -gen-asm-matcher)
tablegen(LLVM I8008GenAsmWriter.inc -gen-asm-writer)
tablegen(LLVM I8008GenCallingConv.inc -gen-callingconv)
tablegen(LLVM I8008GenDAGISel.inc -gen-dag-isel)
tablegen(LLVM I8008GenInstrInfo.inc -gen-instr-info)
tablegen(LLVM I8008GenMCCodeEmitter.inc -gen-emitter)
tablegen(LLVM I8008GenRegisterInfo.inc -gen-register-info)
tablegen(LLVM I8008GenSDNodeInfo.inc -gen-sd-node-info)
tablegen(LLVM I8008GenSubtargetInfo.inc -gen-subtarget)

add_public_tablegen_target(I8008CommonTableGen)

add_llvm_target(I8008CodeGen
  I8008AsmPrinter.cpp
  I8008ExpandPseudos.cpp
  I8008FrameLowering.cpp
  I8008HLTrack.cpp
  I8008ISelDAGToDAG.cpp
  I8008ISelLowering.cpp
  I8008InstrInfo.cpp
  I8008MachineFunctionInfo.cpp
  I8008MCInstLower.cpp
  I8008RegisterInfo.cpp
  I8008SelectionDAGInfo.cpp
  I8008Subtarget.cpp
  I8008TargetMachine.cpp
  I8008TargetTransformInfo.cpp

  LINK_COMPONENTS
  Analysis
  AsmPrinter
  CodeGen
  CodeGenTypes
  Core
  I8008Desc
  I8008Info
  MC
  SelectionDAG
  Support
  Target
  TargetParser

  ADD_TO_COMPONENT
  I8008
  )

add_subdirectory(MCTargetDesc)
add_subdirectory(TargetInfo)
add_subdirectory(AsmParser)
add_subdirectory(Disassembler)
