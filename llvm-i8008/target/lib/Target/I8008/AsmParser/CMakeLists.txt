add_llvm_component_library(LLVMI8008AsmParser
  I8008AsmParser.cpp

  LINK_COMPONENTS
  MC
  MCParser
  I8008Desc
  I8008Info
  Support

  ADD_TO_COMPONENT
  I8008
  )
