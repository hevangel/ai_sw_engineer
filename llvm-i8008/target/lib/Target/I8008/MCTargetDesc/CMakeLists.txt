add_llvm_component_library(LLVMI8008Desc
  I8008AsmBackend.cpp
  I8008ELFObjectWriter.cpp
  I8008InstPrinter.cpp
  I8008MCAsmInfo.cpp
  I8008MCCodeEmitter.cpp
  I8008MCTargetDesc.cpp

  LINK_COMPONENTS
  MC
  I8008Info
  Support

  ADD_TO_COMPONENT
  I8008
  )
