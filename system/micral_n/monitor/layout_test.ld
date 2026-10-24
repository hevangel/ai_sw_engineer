# TEST LAYOUT ONLY (4 KiB of ROM) until the monitor fits 2 KiB.
# MIC 01 monitor image (platform/memory_map.md): 2 KiB ROM at 0x3800 with
# the console entries at 0x3B30 (cold) and 0x3B49 (warm). RAM: see spec MON-10.
CODE  0x2000 0x2000
DATA  0x0700 0x0100
ENTRY mon_cold
PLACE mon_cold 0x3B30
PLACE mon_warm 0x3B49
KEEP  mon_hold0
KEEP  mon_hold1
DEPTH 7
DATAPAGE
