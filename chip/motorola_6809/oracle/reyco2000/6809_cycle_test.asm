; ============================================================
;  MC6809 cycle count test
;  GPL-3.0-or-later, see license.txt
; ============================================================
;
; Checks the number of clock cycles every MC6809 instruction form takes,
; against the Motorola data sheet: each addressing mode, every indexed
; mode's extra cycles, taken and not-taken long branches, the stack
; instructions per byte, and the software interrupts.
;
; Needs the test device's cycle counter (see common.inc): a store to
; DEV_CYC0 marks the start, a store to DEV_CYC1 the end, DEV_CYCD reads the
; difference. Both marks are the same kind of store, so where in its cycles
; a core samples the counter cancels out: the difference is the cycles of
; one "STA extended" (5) plus those of the code measured.
;
; Also valid for an HD6309 in emulation mode, which keeps the 6809 timing.
;
; Every mismatch is reported on the harness console as
;   "$addr: n cycles, expected m"
; where addr is the end of the measured code (look it up in the listing),
; and the run carries on. At the end DEV_EXIT gets the number of
; mismatches (0 = pass); without a test device the program then spins at
; "failed" (or at "success").
;
; Test numbers (TEST_CASE at $0100):
;   $01 inherent              $06 indexed modes (via LDA and LEAX)
;   $02 immediate             $07 indexed indirect modes
;   $03 direct                $08 branches
;   $04 extended              $09 jumps and subroutines
;   $05 read-modify-write     $0A stack, transfer, software interrupts
;
            include "common.inc"
;
VAR         equ SCRATCH
VAR16       equ SCRATCH+2
PTR         equ SCRATCH+4
SAVE_S      equ SCRATCH+6
FAILS       equ SCRATCH+8       ; number of mismatches
GOT         equ SCRATCH+10
WANT        equ SCRATCH+12
DIGIT       equ SCRATCH+14      ; putdec workspace
ANY         equ SCRATCH+15
DPV         equ $0280
DPV16       equ $0282
;
; Start a measurement (clobbers nothing).
cs          macro
            sta DEV_CYC0
            endm
; End a measurement: report it unless the code since "cs" took \1 cycles.
; Clobbers D, X and CC.
ce          macro
            sta DEV_CYC1
            ldd DEV_CYCD
            subd #5
            cmpd #\1
            beq @ok
            ldx #\1
            jsr cyc_fail
@ok         equ *
            endm
; Measure one instruction that has no operand (\1) taking \2 cycles.
cyc         macro
            cs
            \1
            ce \2
            endm
;
            setdp DP_PAGE
            org CODE
start       lds #SSTACK
            ldu #USTACK
            lda #DP_PAGE
            tfr a,dp
            ldx #VAR
            stx PTR
            clr FAILS
            ldx #banner
            jsr puts
            lbra t01
;
banner      fcc "MC6809 cycle count test"
            fcb 13,10,0
puts        lda ,x+
            beq puts_done
            sta DEV_PUTC
            bra puts
puts_done   rts
;
; Report a mismatch: D = cycles measured, X = cycles expected. The return
; address (just past the JSR) identifies the measurement.
cyc_fail    std GOT
            stx WANT
            inc FAILS
            lda #'$'
            sta DEV_PUTC
            ldd ,s
            subd #3             ; the JSR itself
            jsr puthex16
            ldx #msg_got
            jsr puts
            ldd GOT
            jsr putdec
            ldx #msg_want
            jsr puts
            ldd WANT
            jsr putdec
            lda #13
            sta DEV_PUTC
            lda #10
            sta DEV_PUTC
            rts
msg_got     fcn ": "
msg_want    fcn " cycles, expected "
;
; Print D as 4 hex digits.
puthex16    pshs b
            bsr puthex8
            puls a
puthex8     pshs a
            lsra
            lsra
            lsra
            lsra
            bsr puthex4
            puls a
puthex4     anda #$0F
            adda #'0'
            cmpa #'9'
            bls puthex4a
            adda #'A'-'9'-1
puthex4a    sta DEV_PUTC
            rts
;
; Print D (0-65535) in decimal, no leading zeros.
putdec      ldx #dec_pow
            clr ANY             ; a digit has been printed
putdec1     clr DIGIT
putdec2     cmpd ,x
            blo putdec3
            subd ,x
            inc DIGIT
            bra putdec2
putdec3     pshs d
            lda DIGIT
            bne putdec4
            tst ANY
            bne putdec4
            cmpx #dec_pow+8     ; always print the units digit
            bne putdec5
putdec4     adda #'0'
            sta DEV_PUTC
            inc ANY
putdec5     puls d
            leax 2,x
            cmpx #dec_pow+10
            bne putdec1
            rts
dec_pow     fdb 10000,1000,100,10,1
;
; ==========================================================================
; $01 - inherent
; ==========================================================================
t01         test $01
; the measurement itself: nothing between the marks
            cs
            ce 0
            cyc nop,2
            cyc nega,2
            cyc comb,2
            cyc lsra,2
            cyc rorb,2
            cyc asra,2
            cyc aslb,2
            cyc rola,2
            cyc decb,2
            cyc inca,2
            cyc tstb,2
            cyc clra,2
            cyc daa,2
            cyc sex,2
            cyc abx,3
            cyc mul,11
;
; ==========================================================================
; $02 - immediate
; ==========================================================================
t02         test $02
            cs
            lda #1
            ce 2
            cs
            adda #1
            ce 2
            cs
            andb #1
            ce 2
            cs
            cmpa #1
            ce 2
            cs
            ldd #1
            ce 3
            cs
            ldx #1
            ce 3
            cs
            ldu #USTACK
            ce 3
            cs
            ldy #1
            ce 4
            cs
            lds #SSTACK
            ce 4
            cs
            addd #1
            ce 4
            cs
            subd #1
            ce 4
            cs
            cmpx #1
            ce 4
            cs
            cmpd #1
            ce 5
            cs
            cmpy #1
            ce 5
            cs
            cmpu #1
            ce 5
            cs
            cmps #1
            ce 5
            cs
            andcc #$FF
            ce 3
            cs
            orcc #$00
            ce 3
;
; ==========================================================================
; $03 - direct
; ==========================================================================
t03         test $03
            cs
            lda <DPV
            ce 4
            cs
            sta <DPV
            ce 4
            cs
            eorb <DPV
            ce 4
            cs
            ldd <DPV16
            ce 5
            cs
            std <DPV16
            ce 5
            cs
            ldx <DPV16
            ce 5
            cs
            stx <DPV16
            ce 5
            cs
            ldy <DPV16
            ce 6
            cs
            sty <DPV16
            ce 6
            cs
            addd <DPV16
            ce 6
            cs
            cmpx <DPV16
            ce 6
            cs
            cmpd <DPV16
            ce 7
            cs
            cmpu <DPV16
            ce 7
            sts SAVE_S
            cs
            lds <DPV16
            ce 6
            lds SAVE_S
            cs
            sts <DPV16
            ce 6
;
; ==========================================================================
; $04 - extended
; ==========================================================================
t04         test $04
            cs
            lda VAR
            ce 5
            cs
            stb VAR
            ce 5
            cs
            ora VAR
            ce 5
            cs
            ldd VAR16
            ce 6
            cs
            std VAR16
            ce 6
            cs
            ldu VAR16
            ce 6
            cs
            stu VAR16
            ce 6
            ldu #USTACK
            cs
            ldy VAR16
            ce 7
            cs
            sty VAR16
            ce 7
            cs
            subd VAR16
            ce 7
            cs
            cmpx VAR16
            ce 7
            cs
            cmpy VAR16
            ce 8
            cs
            cmps VAR16
            ce 8
;
; ==========================================================================
; $05 - read-modify-write on memory
; ==========================================================================
t05         test $05
            cs
            neg <DPV
            ce 6
            cs
            inc <DPV
            ce 6
            cs
            tst <DPV
            ce 6
            cs
            clr <DPV
            ce 6
            cs
            com VAR
            ce 7
            cs
            lsr VAR
            ce 7
            cs
            tst VAR
            ce 7
            cs
            clr VAR
            ce 7
            ldx #VAR
            cs
            rol ,x
            ce 6
            cs
            dec 1,x
            ce 7
            cs
            clr ,x
            ce 6
;
; ==========================================================================
; $06 - indexed addressing: 4 cycles for LDA plus the mode's extra cycles,
;       4 for LEAX plus the same
; ==========================================================================
t06         test $06
            ldx #VAR
            ldy #VAR
            cs
            lda ,x              ; +0
            ce 4
            cs
            lda 1,y             ; 5 bit +1
            ce 5
            cs
            lda -16,x           ; 5 bit +1
            ce 5
            cs
            lda <1,x            ; 8 bit +1
            ce 5
            cs
            lda >1,x            ; 16 bit +4
            ce 8
            cs
            lda 300,x           ; 16 bit +4
            ce 8
            lda #0
            cs
            lda a,x             ; A,R +1
            ce 5
            ldb #0
            cs
            lda b,x             ; B,R +1
            ce 5
            ldd #0
            cs
            lda d,x             ; D,R +4
            ce 8
            cs
            lda ,x+             ; +2
            ce 6
            cs
            lda ,x++            ; +3
            ce 7
            cs
            lda ,-x             ; +2
            ce 6
            cs
            lda ,--x            ; +3
            ce 7
            cs
            lda <*,pcr          ; 8 bit PCR +1
            ce 5
            cs
            lda >*,pcr          ; 16 bit PCR +5
            ce 9
; LEA
            cs
            leax ,x
            ce 4
            cs
            leay 1,x
            ce 5
            cs
            leau <100,x
            ce 5
            ldu #USTACK
            cs
            leax >100,x
            ce 8
            cs
            leax d,x
            ce 8
            cs
            leax ,x++
            ce 7
            cs
            leas -1,s
            ce 5
            cs
            leas 1,s
            ce 5
; 16 bit loads add the same extra cycles to 5 (LDY: 6)
            ldx #VAR16
            cs
            ldd ,x
            ce 5
            cs
            ldd ,x++
            ce 8
            cs
            ldy ,--x
            ce 9
            cs
            addd ,x
            ce 6
            cs
            cmpd ,x
            ce 7
            cs
            jmp <t06j1,pcr      ; 3 + 1
t06j1       ce 4
;
; ==========================================================================
; $07 - indexed indirect, extended indirect
; ==========================================================================
t07         test $07
            ldx #PTR
            cs
            lda [,x]            ; +3
            ce 7
            cs
            lda [<0,x]          ; 8 bit +4
            ce 8
            cs
            lda [>0,x]          ; 16 bit +7
            ce 11
            lda #0
            cs
            lda [a,x]           ; +4
            ce 8
            ldb #0
            cs
            lda [b,x]           ; +4
            ce 8
            ldd #0
            cs
            lda [d,x]           ; +7
            ce 11
            cs
            lda [,x++]          ; +6
            ce 10
            cs
            lda [,--x]          ; +6
            ce 10
            cs
            lda [<pcr_ptr,pcr]  ; 8 bit PCR +4
            ce 8
            cs
            lda [>pcr_ptr,pcr]  ; 16 bit PCR +8
            ce 12
            cs
            lda [PTR]           ; extended indirect +5
            ce 9
            cs
            ldd [PTR]
            ce 10
            bra t08
pcr_ptr     fdb VAR
;
; ==========================================================================
; $08 - branches
; ==========================================================================
t08         test $08
; "cs" is a STA: with A = 1 it leaves Z clear for the branches below
            andcc #$F0
            cs
            bra t08a
t08a        ce 3
            cs
            brn *
            ce 3
            lda #1
            cs
            beq *               ; not taken
            ce 3
            lda #1
            cs
            bne t08b            ; taken
t08b        ce 3
            cs
            lbra t08c
t08c        ce 5
            cs
            lbrn *
            ce 5
            andcc #$F0
            lda #1
            cs
            lbeq *              ; not taken: 5
            ce 5
            andcc #$F0
            lda #1
            cs
            lbne t08d           ; taken: 6
t08d        ce 6
            orcc #CC_C
            cs
            lbcs t08e
t08e        ce 6
            orcc #CC_C
            cs
            lbcc *
            ce 5
;
; ==========================================================================
; $09 - jumps and subroutines (each with the RTS: 5)
; ==========================================================================
            bra t09
sub         rts                 ; for the BSR/JSR measurements below
t09         test $09
            cs
            jmp t09a
t09a        ce 4
            lda #$7E            ; "JMP t09b" in the direct page
            sta DPV
            ldx #t09b
            stx DPV+1
            cs
            jmp <DPV            ; 3, then the JMP extended: 4
t09b        ce 7
            ldx #t09c
            cs
            jmp ,x
t09c        ce 3
            cs
            bsr sub
            ce 7+5
            cs
            lbsr sub
            ce 9+5
            cs
            jsr sub
            ce 8+5
            ldx #sub
            cs
            jsr ,x
            ce 7+5
            ldd #sub
            std VAR16
            cs
            jsr [VAR16]
            ce 7+5+5
            lda #$39            ; RTS in the direct page
            sta DPV
            cs
            jsr <DPV
            ce 7+5
            lbra t0a
;
; ==========================================================================
; $0A - stack, transfer, software interrupts, RTI
; ==========================================================================
t0a         test $0A
            cs
            pshs a
            ce 6
            cs
            puls a
            ce 6
            cs
            pshs d
            ce 7
            cs
            puls d
            ce 7
            cs
            pshs u,y,x,dp,b,a,cc
            ce 5+10
            cs
            puls cc,a,b,dp,x,y,u
            ce 5+10
            ldu #USTACK
            cs
            pshu y,x
            ce 9
            cs
            pulu x,y
            ce 9
            ldx #t0apc
            pshs x
            cs
            puls pc
t0apc       ce 7
            cs
            tfr x,y
            ce 6
            cs
            exg a,b
            ce 8
            cs
            exg x,y
            ce 8
; RTI with E clear: 6
            ldx #t0arti
            pshs x
            lda #$00
            pshs a
            cs
            rti
t0arti      ce 6
; SWI 19, SWI2 20, SWI3 20, each plus the RTI with E set: 15
            cs
            swi
            ce 19+15
            cs
            swi2
            ce 20+15
            cs
            swi3
            ce 20+15
;
; ==========================================================================
done        lda FAILS
            bne failed
            ldx #passed
            jsr puts
            pass
failed      ldx #msg_failed
            jsr puts
            lda FAILS
            sta DEV_EXIT
failed1     bra failed1
passed      fcc "All tests passed"
            fcb 13,10,0
msg_failed  fcc "Cycle counts differ: see above"
            fcb 13,10,0
;
swi_rti     rti
unexpected  bra unexpected
;
            ifgt *-DEV_LINES
            error "program runs into the test device at $FF00"
            endc
            org $FFF0
            fdb unexpected      ; $FFF0 reserved (HD6309: trap)
            fdb swi_rti         ; $FFF2 SWI3
            fdb swi_rti         ; $FFF4 SWI2
            fdb unexpected      ; $FFF6 FIRQ
            fdb unexpected      ; $FFF8 IRQ
            fdb swi_rti         ; $FFFA SWI
            fdb unexpected      ; $FFFC NMI
            fdb start           ; $FFFE RESET
            end start
