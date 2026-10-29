; ============================================================
;  MC6809 functional test
;  GPL-3.0-or-later, see license.txt
; ============================================================
;
; Tests every documented MC6809 instruction and addressing mode: results,
; every defined condition code bit, and the side effects (stack frames,
; auto increment/decrement, register updates). Undefined flags (H after a
; subtract or shift, V after DAA, ...) are not checked.
;
; Runs unchanged on an HD6309 in emulation mode, which must behave the same.
; Uses no interrupts and needs no test device: a failing check spins on
; itself ("bne *"), success writes 0 to DEV_EXIT and spins at "success".
;
; Assemble:  lwasm --6809 --format=srec --list=bin/6809_functional_test.lst
;                  -o bin/6809_functional_test.s19 src/6809_functional_test.asm
;            (or just "make")
;
; Test numbers (TEST_CASE at $0100) - see the listing for each check:
;   $01 basic branches and CC transfer      $10 8 bit ALU vectors, A and B
;   $02 conditional branches, short          $11 8 bit unary vectors, A/B/mem
;   $03 conditional branches, long           $12 16 bit ADDD/SUBD/CMPx vectors
;   $04 far branches, BSR/LBSR/JSR/RTS/JMP   $13 DAA, all inputs
;   $05 8 bit loads and stores               $14 MUL, SEX, ABX
;   $06 16 bit loads and stores              $15 every opcode, every mode: A/B
;   $07 indexed modes, non-indirect          $16 every opcode, every mode: 16 bit
;   $08 indexed modes, indirect, PCR, [ext]  $17 every opcode, every mode: memory
;   $09 LEAX/LEAY/LEAU/LEAS                  $18 direct page register
;   $0A TFR / EXG                            $19 ANDCC/ORCC, NOP, BRN/LBRN
;   $0B PSHS/PULS/PSHU/PULU                  $1A SWI/SWI2/SWI3/RTI
;
            include "common.inc"
            include "test_macros.inc"
;
; ---- RAM used by this program --------------------------------------------
;
MASK        equ SCRATCH         ; 16 bit branch mask
EXPECT      equ SCRATCH+2
SAVE_S      equ SCRATCH+4
SAVE_X      equ SCRATCH+6
VAR         equ SCRATCH+8       ; extended mode operand
VAR16       equ SCRATCH+10
RES         equ SCRATCH+$20     ; results compared by chk_res ($20 bytes)
SWI_VEC     equ SCRATCH+$40     ; RAM copies of the software interrupt vectors
SWI2_VEC    equ SCRATCH+$42
SWI3_VEC    equ SCRATCH+$44
SWI_FLAG    equ SCRATCH+$46
SWI_S       equ SCRATCH+$48
DPV         equ $0280           ; direct page operand (DP = DP_PAGE)
DPV16       equ $0282
PATTERN     equ $0300           ; $0300-$05FF: byte at a = lo(a) EOR hi(a)
PTRS        equ $0700           ; $0700-$073F: word k = $0300 + k*$11
;
            org CODE
            setdp DP_PAGE
start       lds #SSTACK
            ldu #USTACK
            lda #DP_PAGE
            tfr a,dp
            ldx #banner
            bsr puts
            bra t01
;
; Print the zero-terminated string at X on the harness console.
puts        lda ,x+
            beq puts_done
            sta DEV_PUTC
            bra puts
puts_done   rts
banner      fcc "MC6809 functional test"
            fcb 13,10,0
;
; Compare the B bytes at RES with the bytes at X. TEST_SUB counts the
; bytes, so on a trap it tells which result was wrong.
chk_res     ldy #RES
            clr TEST_SUB
chk_res1    inc TEST_SUB
            lda ,y+
            cmpa ,x+
            bne *
            decb
            bne chk_res1
            rts
;
; ==========================================================================
; $01 - the basics everything else relies on
; ==========================================================================
t01         test $01
            bra t01a
            bra *               ; BRA not taken
t01a        brn *               ; BRN taken
            andcc #$00
            beq *               ; Z clear: BEQ must not branch
            bne t01b
            bra *
t01b        orcc #CC_Z
            bne *               ; Z set: BNE must not branch
            beq t01c
            bra *
t01c        andcc #$00
            tfr cc,a
            tsta
            bne *
            orcc #$FF
            tfr cc,a
            cmpa #$FF
            bne *
            lda #$A5
            tfr a,cc
            tfr cc,b
            cmpb #$A5
            bne *
            lda #$5A
            tfr a,cc
            tfr cc,b
            cmpb #$5A
            bne *
            andcc #$00
            orcc #$C3
            pshs cc
            puls b
            cmpb #$C3
            bne *
            andcc #$00
            ldb #$3C
            pshs b
            puls cc
            tfr cc,a
            cmpa #$3C
            bne *
            lda #$3C
            tfr a,cc
            andcc #$5A
            tfr cc,a
            cmpa #$18
            bne *
            set_cc $00
            chk_cc $00
;
; ==========================================================================
; $02 - all 16 conditional branches, short form, for every N/Z/V/C value
; ==========================================================================
; MASK has bit n set if the branch must be taken when CC = n (CC bits
; 3..0 are N, Z, V, C).
brtest      macro
            ldd #\2
            std MASK
            clrb
@loop       clra
            lsr MASK
            ror MASK+1
            rola
            sta EXPECT
            tfr b,cc
            \1 @taken
            clra
            bra @check
@taken      lda #1
@check      cmpa EXPECT
            bne *               ; branch \1 wrong for CC value in B
            incb
            cmpb #16
            bne @loop
            endm
;
t02         test $02
            brtest bra,$FFFF
            brtest brn,$0000
            brtest bhi,$0505
            brtest bls,$FAFA
            brtest bcc,$5555
            brtest bcs,$AAAA
            brtest bhs,$5555
            brtest blo,$AAAA
            brtest bne,$0F0F
            brtest beq,$F0F0
            brtest bvc,$3333
            brtest bvs,$CCCC
            brtest bpl,$00FF
            brtest bmi,$FF00
            brtest bge,$CC33
            brtest blt,$33CC
            brtest bgt,$0C03
            brtest ble,$F3FC
;
; ==========================================================================
; $03 - the same with the long branches
; ==========================================================================
t03         test $03
            brtest lbra,$FFFF
            brtest lbrn,$0000
            brtest lbhi,$0505
            brtest lbls,$FAFA
            brtest lbcc,$5555
            brtest lbcs,$AAAA
            brtest lbne,$0F0F
            brtest lbeq,$F0F0
            brtest lbvc,$3333
            brtest lbvs,$CCCC
            brtest lbpl,$00FF
            brtest lbmi,$FF00
            brtest lbge,$CC33
            brtest lblt,$33CC
            brtest lbgt,$0C03
            brtest lble,$F3FC
;
; ==========================================================================
; $04 - branch reach, subroutines, jumps
; ==========================================================================
t04         test $04
; short branches at the ends of their range: +127 and -128. The bytes
; skipped are CLRA, so falling into them shows up in A.
            lda #1
            bra t04f127         ; offset +127
            fill $4F,127
t04f127     chk_a 1
            bra t04b128s
t04b128     bra t04b128x
            fill $4F,124
t04b128s    bra t04b128         ; offset -128
t04b128x    chk_a 1
;
; long branches whose 16 bit offsets wrap around the address space
            lbra far_fwd
far_back    lbsr far_sub
            cmpa #$5A
            bne *
; BSR / LBSR push the return address
            lda #1
            bra t04bsr
t04sub      ldx ,s              ; returns its own return address in X
            rts
t04bsr      bsr t04sub
t04ret1     cmpx #t04ret1
            bne *
            lbsr t04sub
t04ret2     cmpx #t04ret2
            bne *
; JSR in every mode
            setdp DP_PAGE
            lda #DP_PAGE
            tfr a,dp
            ldd #t04sub
            std VAR16
            jsr t04sub          ; extended
t04ret3     cmpx #t04ret3
            bne *
            lda #$7E            ; "JMP t04sub" at DPV
            sta DPV
            ldd #t04sub
            std DPV+1
            jsr <DPV            ; direct
t04ret3d    cmpx #t04ret3d
            bne *
            ldx #t04sub
            jsr ,x              ; indexed
t04ret4     cmpx #t04ret4
            bne *
            jsr [VAR16]         ; extended indirect
t04ret5     cmpx #t04ret5
            bne *
; JMP in every mode
            ldx #t04j1
            jmp ,x
            bra *
t04j1       jmp t04j2
            bra *
t04j2       jmp [jvec]
            bra *
jvec        fdb t04j3
t04j3       ldd #t04j4
            std VAR16
            jmp [VAR16]
            bra *
t04j4       jmp t04j5,pcr
            bra *
t04j5       lda #$7E            ; "JMP t04j6" at DPV
            sta DPV
            ldd #t04j6
            std DPV+1
            jmp <DPV            ; direct: lands on the JMP at $0280
            bra *
; RTS via PULS PC, and a return address placed by hand
t04j6       ldd #t04j7
            pshs d
            rts
            bra *
t04j7       ldd #t04j8
            pshs d
            puls pc
            bra *
t04j8       bra t05
;
; ==========================================================================
; $05 - 8 bit loads and stores: flags and every addressing mode
; ==========================================================================
t05         test $05
            set_cc CC_C|CC_V|CC_H|CC_E
            lda #$00            ; Z set, N/V clear, C/H/E kept
            chk_cc CC_C|CC_Z|CC_H|CC_E
            set_cc CC_Z|CC_V
            lda #$80
            chk_cc CC_N
            set_cc CC_N|CC_Z|CC_V|CC_C
            ldb #$7F
            chk_cc CC_C
            set_cc $00
            ldb #$00
            chk_cc CC_Z
; stores set the flags from the stored value
            lda #$80
            set_cc CC_Z|CC_V|CC_C
            sta VAR
            chk_cc CC_N|CC_C
            chk_mem VAR,$80
            clrb
            set_cc CC_N|CC_V
            stb VAR
            chk_cc CC_Z
            chk_mem VAR,$00
; every mode
            lda #$11
            sta >VAR
            ldb >VAR
            chk_b $11
            lda #$22
            sta <DPV
            ldb <DPV
            chk_b $22
            chk_mem DPV,$22
            ldx #VAR
            lda #$33
            sta ,x
            ldb ,x
            chk_b $33
            chk_mem VAR,$33
            ldb #$44
            stb >VAR
            lda >VAR
            chk_a $44
            ldb #$55
            stb <DPV
            lda <DPV
            chk_a $55
            ldb #$66
            stb ,x
            lda ,x
            chk_a $66
;
; ==========================================================================
; $06 - 16 bit loads and stores
; ==========================================================================
t06         test $06
            set_cc CC_C|CC_V|CC_H
            ldd #$0000
            chk_cc CC_C|CC_Z|CC_H
            set_cc CC_Z|CC_V
            ldx #$8000
            chk_cc CC_N
            set_cc CC_N|CC_Z|CC_V|CC_C
            ldy #$7FFF
            chk_cc CC_C
            set_cc CC_N|CC_V
            ldu #$0000
            chk_cc CC_Z
; stores and every mode, register by register
            ldd #$1234
            set_cc CC_N|CC_Z|CC_V
            std VAR16
            chk_cc $00
            chk_mem16 VAR16,$1234
            ldd #$0000
            ldd VAR16
            chk_d $1234
            ldd #$8765
            std <DPV16
            ldd #$0000
            ldd <DPV16
            chk_d $8765
            ldx #VAR16
            ldd #$4321
            std ,x
            ldd #$0000
            ldd ,x
            chk_d $4321
; X
            ldx #$A55A
            set_cc CC_Z|CC_V
            stx VAR16
            chk_cc CC_N
            ldx #$0000
            ldx VAR16
            chk_x $A55A
            ldx #$1357
            stx <DPV16
            ldx #$0000
            ldx <DPV16
            chk_x $1357
            ldy #VAR16
            ldx #$2468
            stx ,y
            ldx #$0000
            ldx ,y
            chk_x $2468
; Y
            ldy #$0000
            set_cc CC_N|CC_V
            sty VAR16
            chk_cc CC_Z
            ldy #$FFFF
            ldy VAR16
            chk_y $0000
            ldy #$369C
            sty <DPV16
            ldy #$0000
            ldy <DPV16
            chk_y $369C
            ldx #VAR16
            ldy #$C963
            sty ,x
            ldy #$0000
            ldy ,x
            chk_y $C963
; U
            pshs u
            ldu #$7001
            stu VAR16
            ldu #$0000
            ldu VAR16
            chk_u $7001
            ldu #$7002
            stu <DPV16
            ldu #$0000
            ldu <DPV16
            chk_u $7002
            ldx #VAR16
            ldu #$7003
            stu ,x
            ldu #$0000
            ldu ,x
            chk_u $7003
            puls u
; S (no stack use while S holds test values)
            sts SAVE_S
            lds #$8001
            sts VAR16
            lds #$0000
            lds VAR16
            chk_s $8001
            lds #$8002
            sts <DPV16
            lds #$0000
            lds <DPV16
            chk_s $8002
            ldx #VAR16
            lds #$8003
            sts ,x
            lds #$0000
            lds ,x
            chk_s $8003
            andcc #$00
            lds #$0000
            beq t06s1
            bra *
t06s1       lds #$8000
            bmi t06s2
            bra *
t06s2       lds SAVE_S
;
; ==========================================================================
; $07 - indexed addressing, non-indirect, with each index register
; ==========================================================================
; Fill $0300-$05FF with lo(a) EOR hi(a) and the pointer table.
t07         test $07
            ldx #PATTERN
t07fill     tfr x,d
            pshs b
            eora ,s+
            sta ,x+
            cmpx #PATTERN+$300
            bne t07fill
            ldx #PTRS
            ldd #$0300
t07ptr      std ,x++
            addd #$0011
            cmpx #PTRS+$40
            bne t07ptr
; spot check the fill: $0480 -> $84, $0354 -> $57
            lda $0480
            cmpa #$84
            bne *
            lda $0354
            cmpa #$57
            bne *
;
; One block per index register. The register starts at $0480; results go
; to RES and are compared with idx_expect. S is used as an index too, so
; the block does not touch the stack until S is restored.
idxblock    macro
            sts SAVE_S
            ld\1 #$0480
            lda ,\1             ; no offset           $0480
            sta RES+0
            lda 15,\1           ; 5 bit +15           $048F
            sta RES+1
            lda -16,\1          ; 5 bit -16           $0470
            sta RES+2
            lda 127,\1          ; 8 bit +127          $04FF
            sta RES+3
            lda -128,\1         ; 8 bit -128          $0400
            sta RES+4
            lda 300,\1          ; 16 bit +300         $05AC
            sta RES+5
            lda -300,\1         ; 16 bit -300         $0354
            sta RES+6
            lda <2,\1           ; 8 bit form, +2      $0482
            sta RES+7
            lda >-2,\1          ; 16 bit form, -2     $047E
            sta RES+8
            lda #$10
            lda a,\1            ; A offset +$10       $0490
            sta RES+9
            lda #$F0
            lda a,\1            ; A offset -$10       $0470
            sta RES+10
            ldb #$7F
            lda b,\1            ; B offset +$7F       $04FF
            sta RES+11
            ldb #$80
            lda b,\1            ; B offset -$80       $0400
            sta RES+12
            ldd #$0100
            lda d,\1            ; D offset +$100      $0580
            sta RES+13
            ldd #$FF00
            lda d,\1            ; D offset -$100      $0380
            sta RES+14
            lda ,\1+            ; post increment      $0480, -> $0481
            sta RES+15
            lda ,\1+            ;                     $0481, -> $0482
            sta RES+16
            lda ,\1++           ; post increment 2    $0482, -> $0484
            sta RES+17
            lda ,-\1            ; pre decrement       $0483
            sta RES+18
            lda ,--\1           ; pre decrement 2     $0481
            sta RES+19
            st\1 VAR16          ; final register value: $0481
            ldd ,\1             ; 16 bit read         $0481/$0482
            std RES+20
            ldd #$BEEF
            std ,\1++           ; 16 bit store, -> $0483
            ldd -2,\1
            std RES+22
            ld\1 #$0480
            ldd #$CAFE
            std -2,\1           ; restore of nothing: just a store
            ldd $047E
            std RES+24
            ldd #$7A7B          ; put the pattern back
            std $047E
            ldd #$8586
            std $0481
            lds SAVE_S
            ldd VAR16
            cmpd #$0481
            bne *
            ldx #idx_expect
            ldb #26
            jsr chk_res
            endm
;
            idxblock x
            idxblock y
            idxblock u
            idxblock s
            bra t08
;
idx_expect  fcb $84,$8B,$74,$FB,$04,$A9,$57,$86,$7A,$94,$74,$FB,$04,$85,$83
            fcb $84,$85,$86,$87,$85
            fcb $85,$86         ; ldd ,r at $0481
            fcb $BE,$EF
            fcb $CA,$FE
;
; ==========================================================================
; $08 - indexed indirect, PC relative, extended indirect
; ==========================================================================
; The register starts at $0720 (in the pointer table). Pointer word k at
; $0700+2k holds $0300+k*$11, so:
;   [$0720] -> $0410 -> $14      [$0724] -> $0432 -> $36
;   [$0718] -> $03CC -> $CF      [$071E] -> $03FF -> $FC
;   [$0730] -> $0498 -> $9C      [$0722] -> $0421 -> $25
t08         test $08
indblock    macro
            sts SAVE_S
            ld\1 #$0720
            lda [,\1]           ; no offset           -> $14
            sta RES+0
            lda [4,\1]          ; 8 bit offset        -> $36
            sta RES+1
            lda [-8,\1]         ; 8 bit negative      -> $CF
            sta RES+2
            lda [>$10,\1]       ; 16 bit offset       -> $9C
            sta RES+3
            lda #4
            lda [a,\1]          ; A offset            -> $36
            sta RES+4
            ldb #$F8
            lda [b,\1]          ; B offset (-8)       -> $CF
            sta RES+5
            ldd #$FFFE
            lda [d,\1]          ; D offset (-2)       -> $FC
            sta RES+6
            lda [,\1++]         ; post increment 2    -> $14, -> $0722
            sta RES+7
            lda [,\1]           ;                     -> $25
            sta RES+8
            lda [,--\1]         ; pre decrement 2     -> $14, -> $0720
            sta RES+9
            lda [,--\1]         ;                     -> $FC, -> $071E
            sta RES+10
            st\1 VAR16
            lds SAVE_S
            ldd VAR16
            cmpd #$071E
            bne *
            ldx #ind_expect
            ldb #11
            jsr chk_res
            endm
;
            indblock x
            indblock y
            indblock u
            indblock s
; PC relative, 8 and 16 bit offsets, direct and indirect
            lda <pcr_byte,pcr
            cmpa #$C3
            bne *
            lda >pcr_byte,pcr
            cmpa #$C3
            bne *
            lda pcr_far,pcr
            cmpa #$3C
            bne *
            lda [pcr_ptr,pcr]
            cmpa #$36           ; via $0432
            bne *
            lda [>pcr_ptr,pcr]
            cmpa #$36
            bne *
            ldx #0
            leax <pcr_byte,pcr
            cmpx #pcr_byte
            bne *
            leax >pcr_far,pcr
            cmpx #pcr_far
            bne *
            leax 2,pc           ; plain PC offset: PC after this instruction + 2
pcr_pc      cmpx #pcr_pc+2
            bne *
; extended indirect
            lda [$0724]
            cmpa #$36
            bne *
            ldd [$0730]         ; $0498/$0499 -> $9C,$9D
            cmpd #$9C9D
            bne *
            bra t09
pcr_byte    fcb $C3
pcr_ptr     fdb $0432
ind_expect  fcb $14,$36,$CF,$9C,$36,$CF,$FC,$14,$25,$14,$FC
;
; ==========================================================================
; $09 - LEA: effective address only; LEAX/LEAY set Z, LEAU/LEAS no flags
; ==========================================================================
t09         test $09
            ldx #$1000
            set_cc CC_N|CC_V|CC_C
            leax -16,x
            chk_cc CC_N|CC_V|CC_C
            chk_x $0FF0
            leax $F010,x        ; wraps to $0000: Z set, others kept
            chk_cc CC_N|CC_V|CC_C|CC_Z
            chk_x $0000
            ldy #$0001
            set_cc CC_Z
            leay -1,y
            chk_cc CC_Z
            leay 1,y
            chk_cc $00
            chk_y $0001
            ldu #$0000
            set_cc $00
            leau ,u             ; LEAU: no flags even though the result is 0
            chk_cc $00
            ldu #$4000
            set_cc CC_Z|CC_N
            leau -1,u
            chk_cc CC_Z|CC_N
            chk_u $3FFF
            ldu #USTACK
            sts SAVE_S
            lds #$0C00
            set_cc CC_Z
            leas -$0C00,s       ; S = 0: no flags
            tfr cc,a
            sts VAR16
            lds SAVE_S
            cmpa #CC_Z
            bne *
            ldd VAR16
            cmpd #$0000
            bne *
; all the computed modes
            ldx #$0480
            lda #$10
            leay a,x
            chk_y $0490
            ldb #$FF
            leay b,x
            chk_y $047F
            ldd #$8000
            leay d,x
            chk_y $8480
            leay ,x++
            chk_y $0480
            chk_x $0482
            leay ,-x
            chk_y $0481
            chk_x $0481
            leay [$0720]
            chk_y $0410
            leax [,x]           ; X <- word at $0481 ($8586)
            chk_x $8586
; LEA used for stack arithmetic
            ldd #$1234
            pshs d
            leas 2,s
            ldd -2,s
            chk_d $1234
;
; ==========================================================================
; $0A - TFR and EXG between registers of the same size
; ==========================================================================
t0a         test $0A
tfrcheck    macro               ; TFR \1,\2 (16 bit, neither is S or PC)
            ld\1 #$A001
            ld\2 #$5002
            set_cc CC_N|CC_C
            tfr \1,\2
            chk_cc CC_N|CC_C
            cmp\2 #$A001
            bne *
            cmp\1 #$A001
            bne *
            endm
exgcheck    macro               ; EXG \1,\2
            ld\1 #$A001
            ld\2 #$5002
            set_cc CC_Z|CC_V
            exg \1,\2
            chk_cc CC_Z|CC_V
            cmp\2 #$A001
            bne *
            cmp\1 #$5002
            bne *
            endm
            pshs u
            tfrcheck d,x
            tfrcheck d,y
            tfrcheck d,u
            tfrcheck x,d
            tfrcheck x,y
            tfrcheck x,u
            tfrcheck y,d
            tfrcheck y,x
            tfrcheck y,u
            tfrcheck u,d
            tfrcheck u,x
            tfrcheck u,y
            exgcheck d,x
            exgcheck d,y
            exgcheck d,u
            exgcheck x,y
            exgcheck x,u
            exgcheck y,u
            exgcheck x,d
            exgcheck u,y
            puls u
; S
            sts SAVE_S
            ldx #$1234
            tfr x,s
            tfr s,y
            lds SAVE_S
            cmpy #$1234
            bne *
            ldx #$4321
            exg x,s
            stx VAR16
            exg x,s
            cmps SAVE_S
            bne *
            cmpx #$4321
            bne *
            ldd VAR16
            cmpd SAVE_S
            bne *
; PC: TFR PC,r gives the address of the next instruction; TFR r,PC jumps
            tfr pc,x
t0apc       cmpx #t0apc
            bne *
            ldy #t0ajmp
            tfr y,pc
            bra *
t0ajmp      ldx #t0aexg
            exg x,pc            ; jump, X <- address after the EXG
t0aexgret   bra *
t0aexg      cmpx #t0aexgret
            bne *
; 8 bit: A, B, CC, DP
            lda #$12
            ldb #$34
            tfr a,b
            chk_d $1212
            ldb #$56
            tfr b,a
            chk_d $5656
            lda #$12
            ldb #$34
            exg a,b
            chk_d $3412
            lda #$F5
            tfr a,cc
            chk_cc $F5
            andcc #$00
            ldb #$0A
            exg b,cc
            chk_cc $0A
            chk_b $00
            andcc #$00
            lda #$33
            tfr a,dp
            tfr dp,b
            chk_b $33
            lda #$44
            exg a,dp
            chk_a $33
            chk_dp $44
            lda #DP_PAGE
            tfr a,dp
;
; ==========================================================================
; $0B - PSHS / PULS / PSHU / PULU
; ==========================================================================
t0b         test $0B
; push everything onto S and check the layout
            sts SAVE_S
            ldu #$5555
            ldy #$4444
            ldx #$3333
            lda #$22
            tfr a,dp
            ldd #$1122
            set_cc $C5
            pshs pc,u,y,x,dp,b,a,cc
t0bpc       tfr cc,a            ; flags unaffected by PSHS
            cmpa #$C5
            bne *
            tfr s,x
            ldd SAVE_S
            subd #12
            stx VAR16
            cmpd VAR16
            bne *
            ldx #t0b_s
            ldb #12
            tfr s,y
            clr TEST_SUB
t0bcmp      inc TEST_SUB
            lda ,y+
            cmpa ,x+
            bne *
            decb
            bne t0bcmp
; pull back in pieces
            ldd #0
            tfr a,dp
            ldx #0
            ldy #0
            ldu #0
            andcc #$00
            puls cc             ; CC first
            chk_cc $C5
            andcc #$00
            puls a,b
            chk_d $1122
            puls dp
            chk_dp $22
            puls x,y
            chk_x $3333
            chk_y $4444
            puls u
            chk_u $5555
            ldx ,s              ; the PC pushed above
            cmpx #t0bpc
            bne *
            leas 2,s
            cmps SAVE_S
            bne *
            lda #DP_PAGE
            tfr a,dp
; PSHU / PULU with S inside the list
            ldu #USTACK
            sts SAVE_S
            ldd #$ABCD
            ldx #$1111
            pshu s,x,d
            cmpu #USTACK-6
            bne *
            ldd ,u
            cmpd #$ABCD
            bne *
            ldd 2,u
            cmpd #$1111
            bne *
            ldd 4,u
            cmpd SAVE_S
            bne *
            ldd #$0000
            ldx #$0000
            pulu d,x
            chk_d $ABCD
            chk_x $1111
            ldd #$7000
            std ,u              ; replace the saved S
            pulu s
            sts VAR16
            lds SAVE_S
            ldd VAR16
            cmpd #$7000
            bne *
            cmpu #USTACK
            bne *
; PULS PC returns; PULU PC as well
            ldx #t0bu
            pshu x
            pulu pc
            bra *
t0bu        ldd #t0bs
            pshs d
            puls pc
            bra *
t0bs        lbra t10
;
t0b_s       fcb $C5,$11,$22,$22,$33,$33,$44,$44,$55,$55
            fdb t0bpc
;
; ==========================================================================
; $10 - 8 bit two-operand ALU instructions against the vector tables
; ==========================================================================
; Each vector: A, M, CCin, A result, CC result; the operand is read with
; ",y" addressing (the other modes are covered by test $15).
t10         test $10
            alu8a adda,v_add,v_add_end,m_add
            alu8a adca,v_adc,v_adc_end,m_adc
            alu8a suba,v_sub,v_sub_end,m_sub
            alu8a sbca,v_sbc,v_sbc_end,m_sbc
            alu8a cmpa,v_cmp,v_cmp_end,m_cmp
            alu8a anda,v_and,v_and_end,m_and
            alu8a ora,v_or,v_or_end,m_or
            alu8a eora,v_eor,v_eor_end,m_eor
            alu8a bita,v_bit,v_bit_end,m_bit
            inc TEST_SUB
            alu8b addb,v_add,v_add_end,m_add
            alu8b adcb,v_adc,v_adc_end,m_adc
            alu8b subb,v_sub,v_sub_end,m_sub
            alu8b sbcb,v_sbc,v_sbc_end,m_sbc
            alu8b cmpb,v_cmp,v_cmp_end,m_cmp
            alu8b andb,v_and,v_and_end,m_and
            alu8b orb,v_or,v_or_end,m_or
            alu8b eorb,v_eor,v_eor_end,m_eor
            alu8b bitb,v_bit,v_bit_end,m_bit
;
; ==========================================================================
; $11 - 8 bit one-operand instructions on A, B and memory
; ==========================================================================
t11         test $11
            un8a nega,v_neg,v_neg_end,m_neg
            un8a coma,v_com,v_com_end,m_com
            un8a lsra,v_lsr,v_lsr_end,m_lsr
            un8a rora,v_ror,v_ror_end,m_ror
            un8a asra,v_asr,v_asr_end,m_asr
            un8a asla,v_asl,v_asl_end,m_asl
            un8a lsla,v_asl,v_asl_end,m_asl
            un8a rola,v_rol,v_rol_end,m_rol
            un8a deca,v_dec,v_dec_end,m_dec
            un8a inca,v_inc,v_inc_end,m_inc
            un8a tsta,v_tst,v_tst_end,m_tst
            un8a clra,v_clr,v_clr_end,m_clr
            inc TEST_SUB
            un8b negb,v_neg,v_neg_end,m_neg
            un8b comb,v_com,v_com_end,m_com
            un8b lsrb,v_lsr,v_lsr_end,m_lsr
            un8b rorb,v_ror,v_ror_end,m_ror
            un8b asrb,v_asr,v_asr_end,m_asr
            un8b aslb,v_asl,v_asl_end,m_asl
            un8b rolb,v_rol,v_rol_end,m_rol
            un8b decb,v_dec,v_dec_end,m_dec
            un8b incb,v_inc,v_inc_end,m_inc
            un8b tstb,v_tst,v_tst_end,m_tst
            un8b clrb,v_clr,v_clr_end,m_clr
            inc TEST_SUB
            un8m neg,v_neg,v_neg_end,m_neg
            un8m com,v_com,v_com_end,m_com
            un8m lsr,v_lsr,v_lsr_end,m_lsr
            un8m ror,v_ror,v_ror_end,m_ror
            un8m asr,v_asr,v_asr_end,m_asr
            un8m asl,v_asl,v_asl_end,m_asl
            un8m rol,v_rol,v_rol_end,m_rol
            un8m dec,v_dec,v_dec_end,m_dec
            un8m inc,v_inc,v_inc_end,m_inc
            un8m tst,v_tst,v_tst_end,m_tst
            un8m clr,v_clr,v_clr_end,m_clr
;
; ==========================================================================
; $12 - 16 bit arithmetic and compares
; ==========================================================================
t12         test $12
            alu16d addd,v_addd,v_addd_end,m_addd
            alu16d subd,v_subd,v_subd_end,m_subd
            alu16d cmpd,v_cmp16,v_cmp16_end,m_cmp16
; CMPX (Y walks the table)
            ldy #v_cmp16
t12x        ldx ,y
            lda 4,y
            tfr a,cc
            cmpx 2,y
            tfr cc,a
            cmpx 5,y
            bne *
            eora 7,y
            anda #m_cmp16
            bne *
            leay 8,y
            cmpy #v_cmp16_end
            bne t12x
; CMPY (X walks the table)
            ldx #v_cmp16
t12y        ldy ,x
            lda 4,x
            tfr a,cc
            cmpy 2,x
            tfr cc,a
            cmpy 5,x
            bne *
            eora 7,x
            anda #m_cmp16
            bne *
            leax 8,x
            cmpx #v_cmp16_end
            bne t12y
; CMPU
            pshs u
            ldy #v_cmp16
t12u        ldu ,y
            lda 4,y
            tfr a,cc
            cmpu 2,y
            tfr cc,a
            cmpu 5,y
            bne *
            eora 7,y
            anda #m_cmp16
            bne *
            leay 8,y
            cmpy #v_cmp16_end
            bne t12u
            puls u
; CMPS (no stack use while S holds the test value)
            sts SAVE_S
            ldy #v_cmp16
t12s        lds ,y
            lda 4,y
            tfr a,cc
            cmps 2,y
            tfr cc,a
            cmps 5,y
            bne *
            eora 7,y
            anda #m_cmp16
            bne *
            leay 8,y
            cmpy #v_cmp16_end
            bne t12s
            lds SAVE_S
;
; ==========================================================================
; $13 - DAA for every A value and every H/C combination
; ==========================================================================
t13         test $13
            un8a daa,v_daa,v_daa_end,m_daa
; and the way it is meant to be used: BCD adds
            lda #$19
            adda #$28
            daa
            chk_a $47
            lda #$99
            adda #$01
            daa
            chk_a $00
            chk_ccm CC_Z|CC_C,CC_Z|CC_C
            lda #$58
            andcc #$FE
            adca #$46
            daa
            chk_a $04
            chk_ccm CC_C,CC_C
;
; ==========================================================================
; $14 - MUL, SEX, ABX
; ==========================================================================
t14         test $14
            ldy #v_mul
t14mul      lda 2,y
            pshs a
            ldd ,y
            puls cc
            mul
            pshs cc
            cmpd 3,y
            bne *
            puls a
            eora 5,y
            anda #m_mul
            bne *
            leay 6,y
            cmpy #v_mul_end
            bne t14mul
; SEX: A <- sign of B; N and Z from D (V is not checked)
            ldd #$007F
            set_cc CC_N|CC_Z|CC_C|CC_H
            sex
            chk_d $007F
            chk_ccm CC_C|CC_H,CC_N|CC_Z|CC_C|CC_H
            ldd #$0080
            set_cc CC_Z
            sex
            chk_d $FF80
            chk_ccm CC_N,CC_N|CC_Z|CC_C|CC_H
            ldd #$FF00
            set_cc CC_N|CC_C
            sex
            chk_d $0000
            chk_ccm CC_Z|CC_C,CC_N|CC_Z|CC_C|CC_H
            ldd #$12FF
            sex
            chk_d $FFFF
; ABX: X <- X + B, unsigned, no flags
            ldx #$1000
            ldb #$FF
            set_cc $0F
            abx
            chk_cc $0F
            chk_x $10FF
            ldx #$FFF0
            ldb #$20
            set_cc $00
            abx
            chk_cc $00
            chk_x $0010
;
; ==========================================================================
; $15 - every 8 bit accumulator opcode in every addressing mode
; ==========================================================================
t15         test $15
            mode8a suba,$50,$60,$F0,CC_N|CC_C
            mode8a cmpa,$50,$50,$50,CC_Z
            mode8a sbca,$50,$51,$FF,CC_N|CC_C
            mode8a anda,$F3,$3C,$30,$00
            mode8a bita,$F0,$0F,$F0,CC_Z
            mode8a lda,$00,$80,$80,CC_N
            mode8a eora,$FF,$0F,$F0,CC_N
            mode8a adca,$7F,$01,$80,CC_N|CC_V
            mode8a ora,$00,$00,$00,CC_Z
            mode8a adda,$FF,$01,$00,CC_Z|CC_C
            mode8b subb,$50,$60,$F0,CC_N|CC_C
            mode8b cmpb,$50,$50,$50,CC_Z
            mode8b sbcb,$50,$51,$FF,CC_N|CC_C
            mode8b andb,$F3,$3C,$30,$00
            mode8b bitb,$F0,$0F,$F0,CC_Z
            mode8b ldb,$00,$80,$80,CC_N
            mode8b eorb,$FF,$0F,$F0,CC_N
            mode8b adcb,$7F,$01,$80,CC_N|CC_V
            mode8b orb,$00,$00,$00,CC_Z
            mode8b addb,$FF,$01,$00,CC_Z|CC_C
; ADC / SBC with carry in, all four modes (immediate covered above)
            lda #$10
            sta VAR
            lda #$20
            orcc #CC_C
            adca >VAR
            chk_a $31
            orcc #CC_C
            sbca >VAR
            chk_a $20
            ldb #$10
            orcc #CC_C
            adcb #$10
            chk_b $21
            orcc #CC_C
            sbcb #$10
            chk_b $10
; STA / STB direct, indexed, extended
            lda #$5A
            clr <DPV
            sta <DPV
            chk_mem DPV,$5A
            ldx #VAR
            clr VAR
            lda #$A5
            sta ,x
            chk_mem VAR,$A5
            ldb #$C3
            stb >VAR
            chk_mem VAR,$C3
            ldb #$3C
            stb <DPV
            chk_mem DPV,$3C
            ldb #$96
            stb ,x
            chk_mem VAR,$96
;
; ==========================================================================
; $16 - every 16 bit opcode in every addressing mode
; ==========================================================================
t16         test $16
            mode16 ldd,subd,cmpd,$1000,$2000,$F000,CC_N|CC_C
            mode16 ldd,addd,cmpd,$7FFF,$0001,$8000,CC_N|CC_V
            mode16 ldd,cmpd,cmpd,$8000,$8000,$8000,CC_Z
            mode16 ldd,ldd,cmpd,$1111,$0000,$0000,CC_Z
            mode16 ldy,cmpy,cmpy,$0001,$0002,$0001,CC_N|CC_C
            mode16 ldy,ldy,cmpy,$0000,$C000,$C000,CC_N
            pshs u
            mode16 ldu,cmpu,cmpu,$8000,$0001,$8000,CC_V
            mode16 ldu,ldu,cmpu,$0000,$1234,$1234,$00
            puls u
; CMPX / LDX without X as the pointer (Y points to VAR16)
            ldd #$4000
            std VAR16
            std <DPV16
            ldy #VAR16
            ldx #$4000
            cmpx #$4000
            chk_ccm CC_Z,$0F
            ldx #$3FFF
            cmpx <DPV16
            chk_ccm CC_N|CC_C,$0F
            ldx #$4001
            cmpx ,y
            chk_ccm $00,$0F
            ldx #$C000
            cmpx >VAR16
            chk_ccm CC_N,$0F
            ldx #0
            ldx <DPV16
            chk_x $4000
            ldx #0
            ldx ,y
            chk_x $4000
            ldx #0
            ldx >VAR16
            chk_x $4000
; CMPS / LDS (CC is parked in A and B while S holds test values)
            sts SAVE_S
            lds #$4000
            cmps #$4000
            tfr cc,a
            lds #$3FFF
            cmps <DPV16
            tfr cc,b
            lds SAVE_S
            anda #$0F
            cmpa #CC_Z
            bne *
            andb #$0F
            cmpb #CC_N|CC_C
            bne *
            lds #$4001
            cmps ,y
            tfr cc,a
            lds #$C000
            cmps >VAR16
            tfr cc,b
            lds SAVE_S
            anda #$0F
            cmpa #$00
            bne *
            andb #$0F
            cmpb #CC_N
            bne *
            lds <DPV16
            cmps #$4000
            bne *
            lds #0
            lds ,y
            cmps #$4000
            bne *
            lds #0
            lds >VAR16
            cmps #$4000
            bne *
            lds SAVE_S
;
; ==========================================================================
; $17 - every read-modify-write opcode on memory in every mode
; ==========================================================================
t17         test $17
            modemem neg,$01,$FF,CC_N|CC_C
            modemem com,$0F,$F0,CC_N|CC_C
            modemem lsr,$81,$40,CC_C
            modemem ror,$02,$81,CC_N
            modemem asr,$81,$C0,CC_N|CC_C
            modemem asl,$40,$80,CC_N|CC_V
            modemem lsl,$80,$00,CC_Z|CC_V|CC_C
            modemem rol,$80,$01,CC_V|CC_C
            modemem dec,$80,$7F,CC_V|CC_C
            modemem inc,$FF,$00,CC_Z|CC_C
            modemem tst,$00,$00,CC_Z|CC_C
            modemem clr,$5A,$00,CC_Z
; inherent A/B forms not used by the vector loops
            lda #$80
            lsla
            chk_a $00
            ldb #$01
            lslb
            chk_b $02
;
; ==========================================================================
; $18 - the direct page register
; ==========================================================================
t18         test $18
            lda #$11
            sta $0310
            lda #$22
            sta $0410
            lda #$03
            tfr a,dp
            setdp $03
            lda <$10            ; $0310
            chk_a $11
            lda #$04
            tfr a,dp
            setdp $04
            lda <$10            ; $0410
            chk_a $22
            lda #$33
            sta <$11            ; $0411
            chk_mem $0411,$33
            clr <$11
            chk_mem $0411,$00
            ldd #$ABCD
            std <$20
            chk_mem16 $0420,$ABCD
            lda #$00
            tfr a,dp
            setdp $00
            ldd #$5A5A
            std <$F0            ; $00F0
            chk_mem16 $00F0,$5A5A
            lda #DP_PAGE
            tfr a,dp
            setdp DP_PAGE
;
; ==========================================================================
; $19 - ANDCC / ORCC, NOP, BRN / LBRN
; ==========================================================================
t19         test $19
            andcc #$00
            chk_cc $00
            orcc #$55
            chk_cc $55
            orcc #$AA
            chk_cc $FF
            andcc #$F0
            chk_cc $F0
            andcc #$0F
            chk_cc $00
            orcc #CC_I|CC_F
            ldd #$1234
            set_cc $AA
            nop
            chk_cc $AA
            chk_d $1234
            brn *
            lbrn *
            chk_cc $AA
            set_cc CC_I|CC_F
;
; ==========================================================================
; $1A - SWI, SWI2, SWI3 and RTI
; ==========================================================================
t1a         test $1A
            ldd #swi_handler
            std SWI_VEC
            std SWI2_VEC
            std SWI3_VEC
; SWI stacks the entire state with E set, then sets I and F
            ldd #$1122
            ldx #$3344
            ldy #$5566
            ldu #$7788
            lda #DP_PAGE
            tfr a,dp
            lda #$11
            sts SAVE_S
            set_cc CC_N|CC_C
            swi
swi_ret1    chk_cc CC_E|CC_N|CC_C ; RTI restored the stacked CC, E included
            cmps SAVE_S
            bne *
            chk_d $1122
            chk_x $3344
            chk_y $5566
            chk_u $7788
            chk_dp DP_PAGE
            lda SWI_FLAG        ; CC inside the handler
            cmpa #CC_E|CC_F|CC_I|CC_N|CC_C
            bne *
            ldx #swi_frame1
            jsr chk_frame
; SWI2: entire state, I and F unchanged
            set_cc CC_Z
            swi2
swi_ret2    chk_cc CC_E|CC_Z
            lda SWI_FLAG
            cmpa #CC_E|CC_Z
            bne *
; SWI3: likewise
            set_cc CC_V|CC_I
            swi3
swi_ret3    chk_cc CC_E|CC_V|CC_I
            lda SWI_FLAG
            cmpa #CC_E|CC_V|CC_I
            bne *
; RTI with E clear pulls only CC and PC
            ldd #rti_short_ret
            pshs d
            lda #CC_C|CC_V
            pshs a
            ldd #$CAFE
            rti
            bra *
rti_short_ret
            chk_cc CC_C|CC_V
            chk_d $CAFE
            cmps SAVE_S
            bne *
; RTI with E set pulls everything
            ldd #rti_long_ret
            pshs d              ; PC
            ldd #$0102
            pshs d              ; U
            ldd #$0304
            pshs d              ; Y
            ldd #$0506
            pshs d              ; X
            lda #$07
            pshs a              ; DP
            ldd #$0809
            pshs b,a            ; A=$08 B=$09 (A at the lower address)
            lda #CC_E|CC_Z
            pshs a              ; CC
            rti
            bra *
rti_long_ret
            chk_cc CC_E|CC_Z
            chk_d $0809
            chk_x $0506
            chk_y $0304
            chk_u $0102
            chk_dp $07
            cmps SAVE_S
            bne *
            lda #DP_PAGE
            tfr a,dp
            ldu #USTACK
            lbra done
;
; Records CC and the stacked frame, then returns with RTI.
swi_handler tfr cc,a
            sta SWI_FLAG
            sts SWI_S
            ldx #RES
            ldb #12
            tfr s,y
swi_copy    lda ,y+
            sta ,x+
            decb
            bne swi_copy
            rti
;
; Compare the frame saved by the SWI handler with the 12 bytes at X.
chk_frame   ldd SAVE_S
            subd #12
            cmpd SWI_S
            bne *               ; S in the handler is not 12 bytes lower
            ldb #12
            jmp chk_res
;
swi_frame1  fcb CC_E|CC_N|CC_C,$11,$22,DP_PAGE,$33,$44,$55,$66,$77,$88
            fdb swi_ret1
;
; ==========================================================================
done        ldx #passed
            jsr puts
            pass
passed      fcc "All tests passed"
            fcb 13,10,0
;
; ---- interrupt vectors: anything unexpected traps --------------------------
swi_thunk   jmp [SWI_VEC]
swi2_thunk  jmp [SWI2_VEC]
swi3_thunk  jmp [SWI3_VEC]
unexp_rsvd  bra unexp_rsvd
unexp_firq  bra unexp_firq
unexp_irq   bra unexp_irq
unexp_nmi   bra unexp_nmi
;
            include "vectors_6809.inc"
            ifgt *-$C000
            error "code and tables run into the far branch block at $C000"
            endc
;
; ---- far branch targets: more than 32K away from the code above -----------
            org $C000
far_fwd     lbra far_back
far_sub     lda #$5A
            rts
pcr_far     fcb $3C
;
            ifgt *-DEV_LINES
            error "program runs into the test device at $FF00"
            endc
            org $FFF0
            fdb unexp_rsvd      ; $FFF0 reserved (HD6309: trap)
            fdb swi3_thunk      ; $FFF2 SWI3
            fdb swi2_thunk      ; $FFF4 SWI2
            fdb unexp_firq      ; $FFF6 FIRQ
            fdb unexp_irq       ; $FFF8 IRQ
            fdb swi_thunk       ; $FFFA SWI
            fdb unexp_nmi       ; $FFFC NMI
            fdb start           ; $FFFE RESET
            end start
