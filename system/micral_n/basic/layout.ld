# SCELBAL: loaded by the boot ROM at 0x0000 and run from there
# (platform/memory_map.md); data follows the code. The boot ROM loads up to
# 0x3700, so code, data and frames must end below it.
CODE  0x0000 0x3700
DATA  follow 0x3700
ENTRY start
RESET basic_restart
DEPTH 7
