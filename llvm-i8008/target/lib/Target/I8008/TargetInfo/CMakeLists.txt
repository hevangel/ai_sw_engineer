add_llvm_component_library(LLVMI8008Info
  I8008TargetInfo.cpp

  LINK_COMPONENTS
  MC
  Support

  ADD_TO_COMPONENT
  I8008
  )
