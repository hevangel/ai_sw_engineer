add_llvm_component_library(LLVMI4004Disassembler
  I4004Disassembler.cpp

  LINK_COMPONENTS
  MCDisassembler
  MC
  I4004Desc
  I4004Info
  Support

  ADD_TO_COMPONENT
  I4004
  )
