add_llvm_component_library(LLVMI8086Desc
  I8086AsmBackend.cpp
  I8086ELFObjectWriter.cpp
  I8086InstPrinter.cpp
  I8086MCAsmInfo.cpp
  I8086MCCodeEmitter.cpp
  I8086MCTargetDesc.cpp

  LINK_COMPONENTS
  MC
  I8086Info
  Support

  ADD_TO_COMPONENT
  I8086
  )

add_dependencies(LLVMI8086Desc I8086CommonTableGen)
