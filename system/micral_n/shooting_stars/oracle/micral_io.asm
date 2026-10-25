; Micral N teletype for the original Shooting Stars (oracle build only; see
; scripts/build_original.sh). Replaces the listing's SBC routines after its
; "cpu 8008new" line. The game keeps its state in B C D E and the message
; pointer in H:L across RST 6, and prints "LAI ' '; RST 6; RST 6", so OUTCHAR
; preserves every register: A waits on the stack card (platform io_map.md)
; while the status is polled. Octal, as the rest of the listing (radix 8).

INCHAR:     LAI 1           ;TELETYPE STATUS
            INP 0
            NDI 1           ;CHARACTER RECEIVED?
            JTZ INCHAR
            LAI 0           ;READ IT
            INP 0
            NDI 177         ;7 BITS
            RET

OUTCHAR:    OUT 36          ;PUSH A ON THE STACK CARD (OUT 30 DECIMAL)
OUTWAIT:    LAI 1           ;TELETYPE STATUS
            INP 0
            NDI 200         ;READY TO SEND?
            JTZ OUTWAIT
            INP 6           ;POP A
            OUT 10          ;SEND IT (OUT 8 DECIMAL)
            RET
