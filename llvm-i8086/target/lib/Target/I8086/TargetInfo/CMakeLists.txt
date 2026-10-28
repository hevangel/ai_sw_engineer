add_llvm_component_library(LLVMI8086Info
  I8086TargetInfo.cpp

  LINK_COMPONENTS
  MC
  Support

  ADD_TO_COMPONENT
  I8086
  )
