# Micral N boot ROM: the 256-byte EPROM at 0x0000 (platform/memory_map.md).
# Its frames sit in 0x3700-0x37FF, above what it loads (spec BOOT-3).
CODE  0x0000 0x0100
DATA  0x3700 0x0100
ENTRY start
DATAPAGE
DEPTH 7
