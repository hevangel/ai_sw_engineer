add_llvm_component_group(I8086)

set(LLVM_TARGET_DEFINITIONS I8086.td)

tablegen(LLVM I8086GenAsmMatcher.inc -gen-asm-matcher)
tablegen(LLVM I8086GenAsmWriter.inc -gen-asm-writer)
tablegen(LLVM I8086GenCallingConv.inc -gen-callingconv)
tablegen(LLVM I8086GenDAGISel.inc -gen-dag-isel)
tablegen(LLVM I8086GenInstrInfo.inc -gen-instr-info)
tablegen(LLVM I8086GenRegisterInfo.inc -gen-register-info)
tablegen(LLVM I8086GenSDNodeInfo.inc -gen-sd-node-info)
tablegen(LLVM I8086GenSubtargetInfo.inc -gen-subtarget)

add_public_tablegen_target(I8086CommonTableGen)

add_llvm_target(I8086CodeGen
  I8086AsmPrinter.cpp
  I8086FrameLowering.cpp
  I8086ISelDAGToDAG.cpp
  I8086ISelLowering.cpp
  I8086InstrInfo.cpp
  I8086MachineFunctionInfo.cpp
  I8086MCInstLower.cpp
  I8086RegisterInfo.cpp
  I8086SelectionDAGInfo.cpp
  I8086Subtarget.cpp
  I8086TargetMachine.cpp

  LINK_COMPONENTS
  Analysis
  AsmPrinter
  CodeGen
  CodeGenTypes
  Core
  I8086Desc
  I8086Info
  MC
  SelectionDAG
  Support
  Target
  TargetParser

  ADD_TO_COMPONENT
  I8086
  )

add_subdirectory(MCTargetDesc)
add_subdirectory(TargetInfo)
add_subdirectory(AsmParser)
add_subdirectory(Disassembler)
