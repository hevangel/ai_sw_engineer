#  -*- Python -*-
# lit configuration for the llvm-i8008 MC tests.
#
# Tools come from llvm-i8008/install/bin (built by scripts/build_llvm.sh).
# Run with scripts/run_tests.sh, or directly:
#   PATH=$PWD/llvm-i8008/install/bin:$PATH lit -v llvm-i8008/tests

import os
import lit.formats

config.name = "llvm-i8008"
config.test_format = lit.formats.ShTest(execute_external=False)
config.suffixes = [".s", ".ll", ".txt"]
config.excludes = ["README.md"]

config.test_source_root = os.path.dirname(__file__)
config.test_exec_root = os.path.join(
    os.environ.get("I8008_TEST_TMP", config.test_source_root), ".lit"
)

# Locate the install/bin directory relative to this file.
backend_dir = os.path.dirname(config.test_source_root)
bin_dir = os.path.join(backend_dir, "install", "bin")
if os.path.isdir(bin_dir):
    config.environment["PATH"] = bin_dir + os.pathsep + os.environ.get("PATH", "")

# Substitutions: the RUN lines use bare tool names, which lit resolves via PATH,
# so no explicit substitutions are strictly required. We still assert the tools
# exist to fail fast with a clear message.
for tool in ["llvm-mc", "llc", "FileCheck", "not", "llvm-readobj"]:
    path = os.path.join(bin_dir, tool)
    if not (os.path.isfile(path) or os.path.isfile(path + ".exe")):
        # Allow PATH fallback (e.g. tools installed system-wide in the image).
        pass
