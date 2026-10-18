# Busicom 141-PF I/O map

From the ai_hw_engineer board spec (§3–4), which was validated by running the original ROM through all 42 manual examples.

| Port | Dir | Device | Behavior |
|---|---|---|---|
| ROM0 bit 0 | out | 4003 #0 (keyboard) shift clock | active-low pulse |
| ROM0 bit 1 | out | serial data to 4003 #0 and #1 | keyboard scan bit is active low |
| ROM0 bit 2 | out | 4003 #1/#2 (printer chain) shift clock | active low |
| ROM1 bits 3:0 | in | keyboard matrix column nibble | row selected by 4003 #0 |
| ROM2 bit 0 | in | printer drum index | one pulse per revolution |
| ROM2 bit 3 | in | paper advance button | |
| RAM0 (P0=0) bit 0 | out | print red | latched per line |
| RAM0 bit 1 | out | fire hammers | rising edge prints current drum character in selected columns |
| RAM0 bit 3 | out | advance paper | rising edge |
| RAM1 (P0=1) bit 0/1/2 | out | Memory / Overflow / Negative lamps | |
| TEST pin | in | drum sector sync | toggles every half-spin |

## Keyboard matrix (4003 #0 row, ROM1 column bit 0/1/2/3)

| Row | bit0 | bit1 | bit2 | bit3 |
|---|---|---|---|---|
| 0 | CM | RM | M− | M+ |
| 1 | √ | % | M=− | M=+ |
| 2 | ◇ (ST) | ÷ | × | = |
| 3 | − | + | — | — |
| 4 | 9 | 6 | 3 | . |
| 5 | 8 | 5 | 2 | 00 |
| 6 | 7 | 4 | 1 | 0 |
| 7 | SIGN | EX | CE | C |
| 8 | decimal selector value (0,1,2,3,4,5,6,8) | | | |
| 9 | rounding selector (0 = FL, 1 = 5/4, 8 = IN) | | | |

Key codes used by the oracle JSON are 129 + 4·row + column (e.g. `0` = 156).

## Printer row (20-bit hammer word)

Bits 3–17: 15 numeric columns (digit, `.` or `-` positions on the drum); bit 0: symbol column A; bit 1: symbol column B. Drum: 13 sectors per revolution — numeric `0`–`9`, `.`, `.`, `-`; column A `◇ + − × ÷ M+ M− ^ = √ % C R`; column B `# * Ⅰ Ⅱ Ⅲ M+ M− T K E Ex C M`.
