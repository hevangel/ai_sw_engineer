# RUN: llvm-mc -triple=i4004 --filetype=obj %s -o %t.o
# RUN: llvm-objdump -d %t.o | sed -nE 's/^[[:space:]]*[0-9a-f]+: ([0-9a-f ]+[0-9a-f])[[:space:]]+.*/\1/p' > %t.ref
# Turn the object's byte column into a 0x-prefixed stream the disassembler reads,
# then reassemble it and extract the second object's byte column.
# RUN: sed -E 's/([0-9a-f]{2})/0x\1/g' %t.ref > %t.hex
# RUN: llvm-mc -triple=i4004 --disassemble %t.hex > %t.redis.s
# RUN: llvm-mc -triple=i4004 --filetype=obj %t.redis.s -o %t2.o
# RUN: llvm-objdump -d %t2.o | sed -nE 's/^[[:space:]]*[0-9a-f]+: ([0-9a-f ]+[0-9a-f])[[:space:]]+.*/\1/p' > %t.ref2
# RUN: diff %t.ref %t.ref2
#
# Assemble -> object -> disassemble -> reassemble -> object; the two objects'
# .text byte streams must be byte-for-byte identical for every instruction and
# every two-word form. This is the end-to-end MC<->disassembler round trip
# (spec/isa.md sections 1-4).

        .text
        nop
        jcn     0x0, 0x11
        jcn     0x8, 0x22
        jcn     0xf, 0x33
        fim     p0, 0x00
        fim     p7, 0xff
        src     p3
        fin     p4
        jin     p5
        jun     0x000
        jun     0xfff
        jms     0xabc
        inc     r0
        inc     r15
        isz     r6, 0x44
        add     r1
        sub     r2
        ld      r3
        xch     r4
        bbl     0
        bbl     15
        ldm     9
        wrm
        wmp
        wrr
        wpm
        wr0
        wr1
        wr2
        wr3
        sbm
        rdm
        rdr
        adm
        rd0
        rd1
        rd2
        rd3
        clb
        clc
        iac
        cmc
        cma
        ral
        rar
        tcc
        dac
        tcs
        stc
        daa
        kbp
        dcl
