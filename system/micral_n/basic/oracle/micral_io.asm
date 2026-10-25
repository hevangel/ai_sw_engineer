; Micral N teletype for the original SCELBAL (oracle build only; see
; scripts/build_original.py). Overlaid on the listing's page-0 serial
; routines; everything else in the image is the listing's bytes. Octal.
;
; SCELBAL works in mark-parity ASCII (bit 7 set: CR is 215, DEL 377) and
; expects the input routine to echo, as the SCELBI bit-banged CINP does bit
; by bit. The Micral teletype is 7-bit (platform io_map.md IO-2): input gets
; bit 7 set, output has it cleared (spec BAS-1). Both routines keep C D E H L,
; and leave A, B (zero) and the flags as the originals do.

            radix 8
            ORG 0
            JMP 100         ;RESET: THE LISTING'S ENTRY (JMP EXEC)

            ORG 103
CINP:       LAI 1           ;TELETYPE STATUS
            INP 0
            NDI 1           ;CHARACTER RECEIVED?
            JTZ CINP
            LAI 0           ;READ IT
            INP 0
            ORI 200         ;MARK PARITY
            JMP CPRINT      ;ECHO IT

            ORG 210
CPRINT:     LBA             ;KEEP THE CHARACTER
CPWAIT:     LAI 1           ;TELETYPE STATUS
            INP 0
            NDI 200         ;READY TO SEND?
            JTZ CPWAIT
            LAB
            NDI 177         ;7 BITS
            OUT 10          ;SEND IT (OUT 8 DECIMAL)
            LAB
            RLC             ;CARRY = BIT 7, AS THE BIT LOOP LEAVES IT
            RRC
            LBI 1           ;B = 0, Z SET, AS THE DELAY LOOP LEAVES THEM
            DCB
            RET
