add_llvm_component_library(LLVMI4004Info
  I4004TargetInfo.cpp

  LINK_COMPONENTS
  MC
  Support

  ADD_TO_COMPONENT
  I4004
  )
