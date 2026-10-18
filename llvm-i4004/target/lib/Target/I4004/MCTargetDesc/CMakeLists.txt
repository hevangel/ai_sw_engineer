add_llvm_component_library(LLVMI4004Desc
  I4004AsmBackend.cpp
  I4004ELFObjectWriter.cpp
  I4004InstPrinter.cpp
  I4004MCAsmInfo.cpp
  I4004MCCodeEmitter.cpp
  I4004MCTargetDesc.cpp

  LINK_COMPONENTS
  MC
  I4004Info
  Support

  ADD_TO_COMPONENT
  I4004
  )
