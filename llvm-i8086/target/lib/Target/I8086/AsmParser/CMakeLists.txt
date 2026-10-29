add_llvm_component_library(LLVMI8086AsmParser
  I8086AsmParser.cpp

  LINK_COMPONENTS
  MC
  MCParser
  I8086Desc
  I8086Info
  Support

  ADD_TO_COMPONENT
  I8086
  )
