add_llvm_component_library(LLVMI4004AsmParser
  I4004AsmParser.cpp

  LINK_COMPONENTS
  MC
  MCParser
  I4004Desc
  I4004Info
  Support

  ADD_TO_COMPONENT
  I4004
  )
