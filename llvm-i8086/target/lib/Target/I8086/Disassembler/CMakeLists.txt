add_llvm_component_library(LLVMI8086Disassembler
  I8086Disassembler.cpp

  LINK_COMPONENTS
  MC
  MCDisassembler
  I8086Desc
  I8086Info
  Support

  ADD_TO_COMPONENT
  I8086
  )
