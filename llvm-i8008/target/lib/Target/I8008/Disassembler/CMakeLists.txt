add_llvm_component_library(LLVMI8008Disassembler
  I8008Disassembler.cpp

  LINK_COMPONENTS
  MCDisassembler
  MC
  I8008Desc
  I8008Info
  Support

  ADD_TO_COMPONENT
  I8008
  )
