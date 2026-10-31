; ============================================================
;  MC6809 interrupt test
;  GPL-3.0-or-later, see license.txt
; ============================================================
;
; Tests IRQ, FIRQ and NMI: masking, priority, the stack frames and the CC
; bits each one sets, level (IRQ/FIRQ) versus edge (NMI) sensing, NMI being
; disarmed until S is first loaded, CWAI and SYNC.
;
; Needs the test device (see common.inc): the program raises and drops the
; interrupt lines itself through DEV_LINES, or schedules a change a number
; of cycles ahead with DEV_DLINES / DEV_DELAY so it can happen while the CPU
; waits in CWAI or SYNC.
;
; A CPU samples its interrupt inputs a cycle or so before an instruction
; ends, so an interrupt raised by a store can be taken after the following
; instruction rather than straight away. The tests leave a few NOPs for it.
;
; Runs unchanged on an HD6309 in emulation mode.
;
; Test numbers (TEST_CASE at $0100):
;   $01 NMI ignored until the first LDS     $09 NMI is edge triggered
;   $02 IRQ: frame, CC, vector              $0A NMI beats FIRQ beats IRQ
;   $03 IRQ masked by I                     $0B registers survive an IRQ
;   $04 IRQ is level triggered              $0C stacked PC is the next instruction
;   $05 FIRQ: frame, CC, vector             $0D CWAI with IRQ and with FIRQ
;   $06 FIRQ masked by F, not by I          $0E SYNC, interrupt masked
;   $07 IRQ nested in a FIRQ handler        $0F SYNC, interrupt enabled
;   $08 NMI: frame, CC, ignores I and F     $10 SYNC with the line already active
;
            include "common.inc"
;
IRQ_VEC     equ SCRATCH         ; RAM vectors (the ROM vectors jump through)
FIRQ_VEC    equ SCRATCH+2
NMI_VEC     equ SCRATCH+4
LOGP        equ SCRATCH+6       ; log write pointer
HCC         equ SCRATCH+8       ; CC seen inside the last handler
COUNT       equ SCRATCH+9
SAVE_S      equ SCRATCH+10
FRAME       equ SCRATCH+$10     ; copy of the last interrupt frame (12 bytes)
LOG         equ SCRATCH+$20     ; handler log, one letter per interrupt
;
            org CODE
; ==========================================================================
; $01 - NMI must not be taken before S has been loaded
; ==========================================================================
start       lda #$01
            sta TEST_CASE
            lda #LINE_NMI
            sta DEV_LINES       ; NMI edge with NMI still disarmed
            nop
            nop
            nop
            nop
            nop                 ; NMI_VEC points at a trap until here
            ldx #nmi_late
            stx NMI_VEC
            clr DEV_LINES
            lds #SSTACK         ; arms NMI. Whether the edge seen while
            ldu #USTACK         ; disarmed is taken now is not defined.
            nop
            nop
            nop
            ldx #nmi_handler
            stx NMI_VEC
            ldx #irq_handler
            stx IRQ_VEC
            ldx #firq_handler
            stx FIRQ_VEC
            lda #DP_PAGE
            tfr a,dp
            ldx #banner
            jsr puts
            lbra t02
;
banner      fcc "MC6809 interrupt test"
            fcb 13,10,0
;
puts        lda ,x+
            beq puts_done
            sta DEV_PUTC
            bra puts
puts_done   rts
;
; Clear the log
log_clear   pshs x
            ldx #LOG
            stx LOGP
            clr ,x
            puls x,pc
; Append the letter in A to the log (keeps it zero terminated)
log_add     pshs x
            ldx LOGP
            sta ,x+
            clr ,x
            stx LOGP
            puls x,pc
; Fail unless the log equals the zero terminated string at X
log_check   ldy #LOG
log_check1  lda ,x+
            cmpa ,y+
            bne *               ; log differs: see LOG at $0130
            tsta
            bne log_check1
            rts
;
; ---- interrupt handlers ---------------------------------------------------
;
; IRQ: entire state stacked. Record CC and the frame, drop the line.
irq_handler tfr cc,a
            sta HCC
            ldx #FRAME
            tfr s,y
            ldb #12
irq_copy    lda ,y+
            sta ,x+
            decb
            bne irq_copy
            lda #'I'
            jsr log_add
            lda DEV_LINES
            anda #~LINE_IRQ&$FF
            sta DEV_LINES
            nop
            rti
;
; FIRQ: only CC and PC stacked, so save what the handler uses.
firq_handler
            pshs cc
            pshs y,x,b,a
            lda 6,s
            sta HCC
            ldx #FRAME
            leay 7,s
            ldb #3
firq_copy   lda ,y+
            sta ,x+
            decb
            bne firq_copy
            lda #'F'
            jsr log_add
            lda DEV_LINES
            anda #~LINE_FIRQ&$FF
            sta DEV_LINES
            nop
            puls a,b,x,y
            leas 1,s
            rti
;
; NMI: entire state stacked. The line is left alone (edge triggered).
nmi_handler tfr cc,a
            sta HCC
            ldx #FRAME
            tfr s,y
            ldb #12
nmi_copy    lda ,y+
            sta ,x+
            decb
            bne nmi_copy
            lda #'N'
            jsr log_add
            rti
;
; NMI that arrives right after the first LDS (allowed, see test $01)
nmi_late    inc COUNT
            rti
;
; ==========================================================================
; $02 - IRQ
; ==========================================================================
t02         test $02
            jsr log_clear
            ldd #$1122
            ldx #$3344
            ldy #$5566
            ldu #$7788
            sts SAVE_S
            set_cc CC_F|CC_N|CC_C
            lda #LINE_IRQ
            sta DEV_LINES
            nop
            nop
            nop
            nop
            ldx #log_i
            jsr log_check
            lda HCC             ; E and I set, F as before
            anda #CC_E|CC_F|CC_I
            cmpa #CC_E|CC_F|CC_I
            bne *
            lda FRAME           ; stacked CC has E set, I still clear
            anda #~(CC_N|CC_Z|CC_V|CC_C|CC_H)&$FF
            cmpa #CC_E|CC_F
            bne *
            ldd FRAME+2         ; B, DP
            cmpd #$2200+DP_PAGE
            bne *
            ldd FRAME+4
            cmpd #$3344
            bne *
            ldd FRAME+6
            cmpd #$5566
            bne *
            ldd FRAME+8
            cmpd #$7788
            bne *
            cmps SAVE_S
            bne *
            ldu #USTACK
;
; ==========================================================================
; $03 - IRQ masked by I, taken as soon as I is cleared
; ==========================================================================
t03         test $03
            jsr log_clear
            orcc #CC_I
            andcc #~CC_F&$FF    ; F clear must not let IRQ in
            lda #LINE_IRQ
            sta DEV_LINES
            nop
            nop
            nop
            nop
            nop
            nop
            ldx #log_none
            jsr log_check
            andcc #~CC_I&$FF
            nop
            nop
            nop
            ldx #log_i
            jsr log_check
;
; ==========================================================================
; $04 - IRQ is level triggered: taken again until the line drops
; ==========================================================================
t04         test $04
            jsr log_clear
            clr COUNT
            ldx #irq_thrice
            stx IRQ_VEC
            lda #LINE_IRQ
            sta DEV_LINES
            nop
            nop
            nop
            nop
            ldx #log_iii
            jsr log_check
            ldx #irq_handler
            stx IRQ_VEC
            lbra t05
irq_thrice  lda #'I'
            jsr log_add
            inc COUNT
            lda COUNT
            cmpa #3
            bne irq_thrice1
            clr DEV_LINES
            nop
irq_thrice1 rti
;
; ==========================================================================
; $05 - FIRQ
; ==========================================================================
t05         test $05
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_FIRQ
            sta DEV_LINES
            nop
            nop
            ldx #log_none
            jsr log_check
            sts SAVE_S
            lda #CC_E|CC_I|CC_Z|CC_V    ; F clear: FIRQ goes in
            pshs a
            ldd #$1122
            ldx #$3344
            puls cc             ; CC, D and X all set before the FIRQ
t05nops     nop
            nop
            nop
            nop
            pshs cc
            cmpd #$1122         ; A and B not touched
            bne *
            cmpx #$3344
            bne *
            puls cc
            cmps SAVE_S
            bne *
            ldx #log_f
            jsr log_check
            lda HCC             ; E cleared, F and I set
            anda #CC_E|CC_F|CC_I
            cmpa #CC_F|CC_I
            bne *
            lda FRAME           ; stacked CC: E clear, F clear
            anda #CC_E|CC_F|CC_I
            cmpa #CC_I
            bne *
            ldd FRAME+1         ; stacked PC is in the NOPs above
            cmpd #t05nops
            blo *
            cmpd #t05nops+4
            bhi *
;
; ==========================================================================
; $06 - FIRQ masked by F only
; ==========================================================================
t06         test $06
            jsr log_clear
            orcc #CC_F
            andcc #~CC_I&$FF
            lda #LINE_FIRQ
            sta DEV_LINES
            nop
            nop
            nop
            nop
            ldx #log_none
            jsr log_check
            andcc #~CC_F&$FF
            nop
            nop
            ldx #log_f
            jsr log_check
;
; ==========================================================================
; $07 - an IRQ taken inside a FIRQ handler that clears I
; ==========================================================================
t07         test $07
            jsr log_clear
            ldx #firq_nest
            stx FIRQ_VEC
            orcc #CC_I|CC_F
            lda #LINE_IRQ|LINE_FIRQ
            sta DEV_LINES
            nop
            andcc #~(CC_I|CC_F)&$FF     ; FIRQ first: it has priority
            nop
            nop
            nop
            nop
            ldx #log_fif
            jsr log_check
            ldx #firq_handler
            stx FIRQ_VEC
            lbra t08
firq_nest   pshs a
            lda #'F'
            jsr log_add
            lda #LINE_IRQ       ; drop FIRQ, keep IRQ
            sta DEV_LINES
            andcc #~CC_I&$FF    ; let the IRQ in
            nop
            nop
            nop
            lda #'f'
            jsr log_add
            puls a
            rti
;
; ==========================================================================
; $08 - NMI
; ==========================================================================
t08         test $08
            jsr log_clear
            ldd #$A1B2
            ldx #$C3D4
            sts SAVE_S
            set_cc CC_E|CC_F|CC_I|CC_H  ; masks do not stop NMI
            lda #LINE_NMI
            sta DEV_LINES
            nop
            nop
            nop
            nop
            ldx #log_n
            jsr log_check
            cmps SAVE_S
            bne *
            lda HCC             ; E, F and I set
            anda #CC_E|CC_F|CC_I
            cmpa #CC_E|CC_F|CC_I
            bne *
            lda FRAME
            cmpa #CC_E|CC_F|CC_I|CC_H
            bne *
            lda FRAME+2         ; B
            cmpa #$B2
            bne *
;
; ==========================================================================
; $09 - NMI is edge triggered: a line held high fires once
; ==========================================================================
t09         test $09
            jsr log_clear
            nop                 ; the line is still high from test $08
            nop
            nop
            nop
            ldx #log_none
            jsr log_check
            clr DEV_LINES
            nop
            nop
            ldx #log_none
            jsr log_check
            lda #LINE_NMI
            sta DEV_LINES       ; a new edge
            nop
            nop
            nop
            ldx #log_n
            jsr log_check
            clr DEV_LINES
            nop
;
; ==========================================================================
; $0A - priority: NMI, then FIRQ, then IRQ
; ==========================================================================
t0a         test $0A
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_IRQ|LINE_FIRQ
            sta DEV_LINES
            nop
            lda #LINE_IRQ|LINE_FIRQ|LINE_NMI
            sta DEV_LINES
            nop
            nop
            nop
            ldx #log_n          ; NMI in, FIRQ and IRQ still masked
            jsr log_check
            jsr log_clear
            andcc #~(CC_I|CC_F)&$FF
            nop
            nop
            nop
            nop
            nop
            ldx #log_fi
            jsr log_check
            clr DEV_LINES
            nop
;
; ==========================================================================
; $0B - an IRQ handler may use every register: RTI restores them all
; ==========================================================================
t0b         test $0B
            ldx #irq_trash
            stx IRQ_VEC
            lda #$5A
            tfr a,dp
            ldd #$0102
            ldx #$0304
            ldy #$0506
            ldu #$0708
            sts SAVE_S
            andcc #$00
            lda #LINE_IRQ
            sta DEV_LINES
            lda #$01            ; A back to $01 for the check below
            nop
            nop
            nop
            nop
            chk_d $0102
            chk_x $0304
            chk_y $0506
            chk_u $0708
            chk_dp $5A
            cmps SAVE_S
            bne *
            lda #DP_PAGE
            tfr a,dp
            ldu #USTACK
            ldx #irq_handler
            stx IRQ_VEC
            lbra t0c
irq_trash   clr DEV_LINES
            ldd #$FFFF
            ldx #$FFFF
            ldy #$FFFF
            ldu #$FFFF
            tfr a,dp
            orcc #$0F
            rti
;
; ==========================================================================
; $0C - the stacked PC is the next instruction to run
; ==========================================================================
t0c         test $0C
            jsr log_clear
            andcc #~CC_I&$FF
            lda #LINE_IRQ
            sta DEV_LINES
t0csled     nop
            nop
            nop
            nop
            nop
t0csled_end nop
            ldx #log_i
            jsr log_check
            ldd FRAME+10
            cmpd #t0csled
            blo *
            cmpd #t0csled_end
            bhi *
;
; ==========================================================================
; $0D - CWAI
; ==========================================================================
; CWAI ANDs CC with its operand, stacks the entire state (E set) and waits.
; The interrupt then goes straight to its vector: no second frame.
t0d         test $0D
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_IRQ
            sta DEV_DLINES
            ldd #60
            std DEV_DELAY       ; IRQ in 60 cycles
            ldd #$4455
            ldx #$6677
            sts SAVE_S
            set_cc CC_I|CC_F|CC_N|CC_Z
            cwai #~(CC_I|CC_N)&$FF
t0dret1     tfr cc,a            ; RTI restored CC from the CWAI frame
            cmpa #CC_E|CC_F|CC_Z
            bne *
            cmps SAVE_S
            bne *
            ldx #log_i
            jsr log_check
            lda FRAME           ; CC & operand, then E
            cmpa #CC_E|CC_F|CC_Z
            bne *
            ldd FRAME+1         ; A, B
            cmpd #$4455
            bne *
            ldd FRAME+4
            cmpd #$6677
            bne *
            ldd FRAME+10
            cmpd #t0dret1
            bne *
            lda HCC
            anda #CC_E|CC_F|CC_I
            cmpa #CC_E|CC_F|CC_I ; IRQ sets I, leaves F (set here)
            bne *
; CWAI with FIRQ: the frame is still the entire state, E set
            inc TEST_SUB
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_FIRQ
            sta DEV_DLINES
            ldd #60
            std DEV_DELAY
            ldd #$8899
            ldx #$AABB
            ldy #$CCDD
            sts SAVE_S
            set_cc CC_I|CC_F
            cwai #~CC_F&$FF
t0dret2     chk_cc CC_E|CC_I
            cmps SAVE_S
            bne *
            chk_d $8899         ; restored from the entire frame
            chk_x $AABB
            chk_y $CCDD
            ldx #log_f
            jsr log_check
            lda FRAME           ; the FIRQ handler saw an E=1 frame
            cmpa #CC_E|CC_I
            bne *
            ldd FRAME+1
            cmpd #$8899
            bne *
            lda HCC
            anda #CC_E|CC_F|CC_I
            cmpa #CC_E|CC_F|CC_I
            bne *
; CWAI with the interrupt already waiting: taken at once
            inc TEST_SUB
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_IRQ
            sta DEV_LINES
            nop
            nop
            cwai #~CC_I&$FF
            ldx #log_i
            jsr log_check
;
; ==========================================================================
; $0E - SYNC with the interrupt masked: carries on after SYNC, no vector
; ==========================================================================
t0e         test $0E
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_IRQ
            sta DEV_DLINES
            ldd #100
            std DEV_DELAY
            sta DEV_CYC0
            sync
            sta DEV_CYC1
            ldd DEV_CYCD        ; SYNC really waited for the line
            cmpd #90
            blo *
            ldx #log_none
            jsr log_check
            clr DEV_LINES
            nop
; and with FIRQ masked
            inc TEST_SUB
            lda #LINE_FIRQ
            sta DEV_DLINES
            ldd #100
            std DEV_DELAY
            sync
            ldx #log_none
            jsr log_check
            clr DEV_LINES
            nop
;
; ==========================================================================
; $0F - SYNC with the interrupt enabled: handler, then on after SYNC
; ==========================================================================
t0f         test $0F
            jsr log_clear
            andcc #~CC_I&$FF
            lda #LINE_IRQ
            sta DEV_DLINES
            ldd #80
            std DEV_DELAY
            sync
t0fret      ldx #log_i
            jsr log_check
            ldd FRAME+10
            cmpd #t0fret
            bne *
; NMI ends a SYNC too
            inc TEST_SUB
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_NMI
            sta DEV_DLINES
            ldd #80
            std DEV_DELAY
            sync
            ldx #log_n
            jsr log_check
            clr DEV_LINES
            nop
;
; ==========================================================================
; $10 - SYNC with a masked interrupt line already active: no wait
; ==========================================================================
t10         test $10
            jsr log_clear
            orcc #CC_I|CC_F
            lda #LINE_IRQ
            sta DEV_LINES
            nop
            sta DEV_CYC0
            sync                ; must not wait forever
            sta DEV_CYC1
            ldd DEV_CYCD
            cmpd #40
            bhi *
            ldx #log_none
            jsr log_check
            clr DEV_LINES
            nop
;
; ==========================================================================
done        ldx #passed
            jsr puts
            pass
passed      fcc "All tests passed"
            fcb 13,10,0
;
log_none    fcb 0
log_i       fcn "I"
log_iii     fcn "III"
log_f       fcn "F"
log_fif     fcn "FIf"
log_fi      fcn "FI"
log_n       fcn "N"
;
irq_thunk   jmp [IRQ_VEC]
firq_thunk  jmp [FIRQ_VEC]
nmi_thunk   jmp [NMI_VEC]
nmi_trap    bra nmi_trap        ; NMI before the first LDS
unexpected  bra unexpected
;
            org NMI_VEC
            fdb nmi_trap        ; initial RAM vector, loaded with the program
;
            ifgt *-DEV_LINES
            error "program runs into the test device at $FF00"
            endc
            org $FFF0
            fdb unexpected      ; $FFF0 reserved (HD6309: trap)
            fdb unexpected      ; $FFF2 SWI3
            fdb unexpected      ; $FFF4 SWI2
            fdb firq_thunk      ; $FFF6 FIRQ
            fdb irq_thunk       ; $FFF8 IRQ
            fdb unexpected      ; $FFFA SWI
            fdb nmi_thunk       ; $FFFC NMI
            fdb start           ; $FFFE RESET
            end start
