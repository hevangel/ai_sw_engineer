; ============================================================
;  6809 / 6309 functional test suite - shared definitions
;  GPL-3.0-or-later, see license.txt
; ============================================================
;
; Every test program in this suite follows the same contract, so one
; harness can run all of them on any CPU core:
;
;   * The program is loaded as a Motorola S-record (bin/*.s19). It sets its
;     own vectors at $FFF0-$FFFF; the harness only has to RESET the CPU.
;   * The current test number is kept in RAM at TEST_CASE. When a check
;     fails the program branches to itself ("bne *" etc.), so a failing
;     core ends up spinning on the very instruction that detected the
;     error. Look that address up in the matching bin/*.lst listing.
;   * On success the program writes 0 to DEV_EXIT and then spins on
;     "bra *". A harness without the test device can stop on that loop
;     and compare the PC with the "success" symbol in the listing.
;
; ---- Test device ($FF00-$FF0F) ------------------------------------------
;
; A tiny memory-mapped peripheral the harness provides. Only the interrupt
; and cycle tests need it; the functional tests only use DEV_EXIT and
; DEV_PUTC, which are harmless when missing (writes to plain RAM).
;
DEV_LINES   equ $FF00   ; R/W  interrupt line levels: bit0 IRQ, bit1 FIRQ,
                        ;      bit2 NMI (NMI fires on the 0->1 transition)
DEV_DLINES  equ $FF01   ; R/W  line levels to load into DEV_LINES later...
DEV_DELAY   equ $FF02   ; W    ...after this many CPU cycles (16 bit, big
                        ;      endian, $FF02/$FF03). Writing $FF03 arms it.
DEV_CYC0    equ $FF04   ; W    latch the cycle counter (start mark)
DEV_CYC1    equ $FF05   ; W    latch the cycle counter (end mark)
DEV_CYCD    equ $FF06   ; R    16 bit: end mark - start mark ($FF06/$FF07)
DEV_PUTC    equ $FF08   ; W    write a character to the harness console
DEV_EXIT    equ $FF0F   ; W    stop: 0 = all tests passed, else = failure
;
LINE_IRQ    equ $01
LINE_FIRQ   equ $02
LINE_NMI    equ $04
;
; ---- Memory map shared by all programs ----------------------------------
;
TEST_CASE   equ $0100   ; current test number (byte)
TEST_SUB    equ $0101   ; optional sub-step / loop counter for diagnosis
SCRATCH     equ $0110   ; general purpose scratch RAM $0110-$01FF
DP_PAGE     equ $02     ; direct page used by direct mode tests ($0200-$02FF)
DATA        equ $0300   ; data area $0300-$07FF
USTACK      equ $0A00   ; top of user stack (U)
SSTACK      equ $0C00   ; top of system stack (S)
CODE        equ $1000   ; programs start here
;
; ---- Condition code bits ------------------------------------------------
;
CC_C        equ $01
CC_V        equ $02
CC_Z        equ $04
CC_N        equ $08
CC_I        equ $10
CC_H        equ $20
CC_F        equ $40
CC_E        equ $80
;
; ---- Macros -------------------------------------------------------------
;
; Start test number \1. Leaves A and CC changed.
test        macro
            lda #\1
            sta TEST_CASE
            clr TEST_SUB
            endm
;
; Set CC to exactly \1 without touching any other register.
set_cc      macro
            andcc #$00
            orcc #\1
            endm
;
; Fail unless CC equals \1 exactly. Preserves all registers and CC.
chk_cc      macro
            pshs cc,a
            lda ,s
            cmpa #\1
            bne *
            puls cc,a
            endm
;
; Fail unless (CC & \2) == \1. Preserves all registers and CC.
chk_ccm     macro
            pshs cc,a
            lda ,s
            anda #\2
            cmpa #\1
            bne *
            puls cc,a
            endm
;
; Fail unless register equals the immediate value. Each preserves CC.
chk_a       macro
            pshs cc
            cmpa #\1
            bne *
            puls cc
            endm
chk_b       macro
            pshs cc
            cmpb #\1
            bne *
            puls cc
            endm
chk_d       macro
            pshs cc
            cmpd #\1
            bne *
            puls cc
            endm
chk_x       macro
            pshs cc
            cmpx #\1
            bne *
            puls cc
            endm
chk_y       macro
            pshs cc
            cmpy #\1
            bne *
            puls cc
            endm
chk_u       macro
            pshs cc
            cmpu #\1
            bne *
            puls cc
            endm
; S cannot be checked with pshs/puls around it; use the plain compare.
chk_s       macro
            cmps #\1
            bne *
            endm
chk_dp      macro
            pshs cc,a
            tfr dp,a
            cmpa #\1
            bne *
            puls cc,a
            endm
;
; Fail unless the byte at address \1 equals \2. Preserves registers and CC.
chk_mem     macro
            pshs cc,a
            lda \1
            cmpa #\2
            bne *
            puls cc,a
            endm
chk_mem16   macro
            pshs cc,d
            ldd \1
            cmpd #\2
            bne *
            puls cc,d
            endm
;
; Report success to the harness and stop.
pass        macro
            lda #$FF
            sta TEST_CASE
            clr DEV_EXIT
success     bra success
            endm
