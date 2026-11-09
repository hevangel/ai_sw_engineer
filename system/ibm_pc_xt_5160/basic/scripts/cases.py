#!/usr/bin/env python3
"""The Disk BASIC (BASIC.COM) test cases: BASIC programs (exact bytes) and what is typed.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

BASIC.COM extends Cassette BASIC C1.10 in the XT's ROM (the firmware title) with disk files,
LOAD and SAVE, and more. Each case runs `basic NAME` on an ASCII program that prints its
results and ends with SYSTEM, back at A>; cases that write files then TYPE them. The programs
avoid TIME$, DATE$, TIMER and RND without a seed, whose values depend on the emulator's clock.
The interactive cases first turn the function-key line off (KEY OFF), so `Ok` is the last row,
then type at the `Ok` prompt. A wait text starting with `$` waits for the last
non-blank row to end with the rest, once the screen has changed. The inputs are this
repository's own bytes. SHELL is not covered: on MartyPC, EXIT from the child COMMAND did
not come back to BASIC within the case (to look into with the rewrite).
Cases b01-b09 are the first set; b10-b31 (issue #301 phase 1) cover number formatting, PRINT USING,
strings, trapped errors, every error message, the tokenized and protected SAVE formats, LIST,
the screen editor's control keys, AUTO/RENUM/TRON/CONT, INPUT, the screen functions and KEY,
CHAIN/MERGE/COMMON, file modes, variables and the data segment's layout (spec/spec.md). b29 and b30
end in BASIC (no SYSTEM), so their screen shows the function-key line."""
import sys
from pathlib import Path


def prog(*lines):
    return b"".join(l.encode() + b"\r\n" for l in lines) + b"\x1a"


FILES = {
    "PRINT.BAS": prog(
        '10 PRINT "integers";1+2;7\\2;7 MOD 3;-5',
        '20 PRINT "singles";1/3;2.5E+3;1E-5;123456789',
        '30 PRINT "doubles";1#/3;12345678.9#',
        '40 PRINT "strings ";LEFT$("abcdef",3);MID$("abcdef",2,3);RIGHT$("abcdef",2);LEN("abc")',
        '50 PRINT USING "###.##  $$#,###.##  +#.###^^^^";3.14159;1234.5;-0.000123',
        '60 PRINT "tab";TAB(20);"here";SPC(3);"x"',
        '70 PRINT HEX$(255);" ";OCT$(8);" ";CHR$(65);ASC("a");STR$(42);VAL("12.5")',
        '80 SYSTEM'),
    "LOOPS.BAS": prog(
        '10 DIM A(10),B$(3)',
        '20 FOR I=1 TO 10:A(I)=I*I:NEXT',
        '30 S=0:FOR I=1 TO 10:S=S+A(I):NEXT:PRINT "sum";S',
        '40 FOR I=10 TO 1 STEP -3:PRINT I;:NEXT:PRINT',
        '50 GOSUB 100:GOSUB 100',
        '60 I=0',
        '70 WHILE I<5:I=I+1:WEND:PRINT "while";I',
        '80 IF S>300 THEN PRINT "big" ELSE PRINT "small"',
        '90 ON 2 GOTO 91,92',
        '91 PRINT "one":GOTO 95',
        '92 PRINT "two"',
        '95 READ X,Y$:PRINT X;Y$:RESTORE:READ Z:PRINT Z',
        '96 DATA 7,seven',
        '97 SYSTEM',
        '100 C=C+1:PRINT "sub";C:RETURN'),
    "MATH.BAS": prog(
        '10 PRINT SQR(2);SIN(1);COS(1);ATN(1)*4',
        '20 PRINT EXP(1);LOG(10);INT(-2.5);FIX(-2.5);ABS(-3);SGN(-7)',
        '30 PRINT 2^10;2^.5;CINT(2.5);CINT(3.5);CSNG(1#/3);CDBL(1/3)',
        '40 DEF FNF(X)=X*X+1:PRINT FNF(3)',
        '50 PRINT 1E38*10',
        '60 SYSTEM'),
    "SEQ.BAS": prog(
        '10 OPEN "O",#1,"B:SEQ.DAT"',
        '20 FOR I=1 TO 3:WRITE #1,I,"item"+STR$(I),I/4:NEXT',
        '30 PRINT #1,"plain line"',
        '40 CLOSE #1',
        '50 OPEN "I",#1,"B:SEQ.DAT"',
        '60 FOR I=1 TO 3:INPUT #1,A,B$,C:PRINT A;B$;C:NEXT',
        '70 LINE INPUT #1,L$:PRINT L$;EOF(1)',
        '80 CLOSE',
        '90 OPEN "A",#2,"B:SEQ.DAT":PRINT #2,"appended":CLOSE #2',
        '100 SYSTEM'),
    "RAND.BAS": prog(
        '10 OPEN "R",#1,"B:REC.DAT",20',
        '20 FIELD #1,8 AS N$,4 AS V$,8 AS T$',
        '30 FOR I=1 TO 3:LSET N$="rec"+STR$(I):LSET V$=MKS$(I*1.5):RSET T$="end":PUT #1,I:NEXT',
        '40 GET #1,2:PRINT N$;CVS(V$);T$',
        '50 PRINT LOF(1);LOC(1)',
        '60 CLOSE:SYSTEM'),
    "ERRS.BAS": prog(
        '10 ON ERROR GOTO 100',
        '20 X=1/0',
        '30 PRINT "after";X',
        '40 OPEN "I",#1,"B:NOFILE.DAT"',
        '50 A$=STRING$(300,"x")',
        '60 ON ERROR GOTO 0',
        '70 PRINT "done"',
        '80 SYSTEM',
        '100 PRINT "error";ERR;"line";ERL:RESUME NEXT'),
    "FILES.BAS": prog(
        '10 OPEN "O",#1,"B:ONE.TXT":PRINT #1,"one":CLOSE',
        '15 OPEN "O",#1,"B:KEEP.TXT":PRINT #1,"keep":CLOSE',
        '20 NAME "B:ONE.TXT" AS "B:TWO.TXT"',
        '30 FILES "B:*.*"',
        '40 KILL "B:TWO.TXT"',
        '50 FILES "B:*.*"',
        '60 SYSTEM'),
    "NUMFMT.BAS": prog(
        '10 PRINT 0;-0;1E6;1E7;999999!;9999999!;1E16;1E-7',
        '20 PRINT 123456.7;1234567.8;.1;-.5;1/7;2/3',
        '30 PRINT 1D16;1#/7;100#;1.5D-10;-1D30;12345678901234567#',
        '40 PRINT 32767;-32768;32767+1;&HFF;&O17;&H8000;&HFFFF',
        '50 PRINT 1.0000001;.99999999#;5.000001E+15;1E+38',
        '60 X%=3.7:PRINT X%;-7\\2;7 MOD -3;-7 MOD 3;CINT(-2.5);CINT(-3.5)',
        '70 PRINT STR$(-1.5);STR$(1E20);VAL("&H10");VAL(" 1 2 ");VAL("1E2x");VAL("-.5D1")',
        '80 A#=1/3:B#=1#/3:PRINT A#;B#;CSNG(B#);CDBL(.1)',
        '90 PRINT 2^31;10^-3;0^0;2^-1;(-2)^3;1.5^2.5',
        '100 PRINT 5 AND 3;5 OR 3;5 XOR 3;NOT 5;5 IMP 3;5 EQV 3;3>2;2>3',
        '110 PRINT 1,2,3,4,5,6',
        '120 PRINT 1.234567E-20;-9.999999E+37;123.4567#*1000',
        '130 PRINT 1E10*1E10;1D10*1D10;100000!*100000!',
        '140 RANDOMIZE 7:PRINT RND;RND;RND(0);RND(-3);RND',
        '150 PRINT INT(1E10+.5);FIX(-1E10-.5);INT(-.1);SGN(0);ABS(-32768)',
        '160 SYSTEM'),
    "USING.BAS": prog(
        '10 PRINT USING "##.##";1.005;-1.5;123.456',
        '20 PRINT USING "**#.#";2.3;-2.3',
        '30 PRINT USING "**$##.##";4.5',
        '40 PRINT USING "###.##-";-12.3;12.3',
        '50 PRINT USING "#.##^^^^";12345;-.00012',
        '60 PRINT USING "!";"hello";"world"',
        '70 PRINT USING "\\  \\";"abcdefg";"ab"',
        '80 PRINT USING "&!";"ab";"cd"',
        '90 PRINT USING "_####";5',
        '100 PRINT USING "Total: ###,###.##";1234567.891',
        '110 PRINT USING "##";12345;-5',
        '120 PRINT USING "+##  ##+";5;-5;5;-5',
        '130 PRINT USING "$$###.##";-12.5;12.5',
        '140 PRINT USING ".###";.5555;1',
        '150 PRINT USING "##.##^^^^^";1.5E+30',
        '160 PRINT USING "#.#";1E+38',
        '170 PRINT USING "###.#";9999.95',
        '180 PRINT USING "a#b,";1;2',
        '190 SYSTEM'),
    "STRS.BAS": prog(
        '10 A$="Hello, World"',
        '20 PRINT INSTR(A$,"o");INSTR(6,A$,"o");INSTR(A$,"z");INSTR(A$,"");INSTR(20,A$,"")',
        '30 PRINT STRING$(5,"*");STRING$(3,65);SPACE$(3);"|";SPC(0);"|"',
        '40 MID$(A$,1,5)="HOWDY":PRINT A$:MID$(A$,8)="Earth!!!":PRINT A$',
        '50 PRINT LEFT$(A$,0);"|";RIGHT$(A$,100);"|";MID$(A$,20);"|";MID$(A$,3,0);"|"',
        '60 PRINT "b">"a";"abc"<"abd";"A"="a";"ab"<"abc";"";"x"',
        '70 PRINT HEX$(-1);" ";OCT$(-1);" ";HEX$(4096);" ";HEX$(1.7)',
        '80 PRINT CVI(MKI$(-2));CVD(MKD$(1#/3));LEN(MKS$(1));CVS(MKS$(2.5))',
        '90 S$="":FOR I=1 TO 5:S$=S$+CHR$(64+I):NEXT:PRINT S$;LEN(S$+S$)',
        '100 PRINT ASC(S$);VAL("");STR$(0);LEN(STR$(1));"x";STR$(-0)',
        '110 B$="  pad  ":PRINT "[";B$;"]";LEN(B$)',
        '120 C$=STRING$(255,"z"):PRINT LEN(C$);RIGHT$(C$,3)',
        '130 PRINT FRE(0);FRE("");FRE(1)',
        '140 DIM D$(2):D$(1)="one":SWAP D$(1),D$(2):PRINT "[";D$(1);"][";D$(2);"]"',
        '150 SYSTEM'),
    "TRAP.BAS": prog(
        '10 ON ERROR GOTO 1000',
        '20 PRINT ASC("")',
        '30 RETURN',
        '40 NEXT',
        '50 READ X',
        '60 DIM A(5):A(6)=1',
        '70 DIM A(3)',
        '80 X%=40000',
        '90 Y=FNZ(1)',
        '100 ERROR 200',
        '110 PRINT LEFT$("a",-1)',
        '120 PRINT #3,"x"',
        '130 OPEN "X",#1,"B:A"',
        '140 WEND',
        '150 A$="x"+1',
        '160 GOTO 9999',
        '170 X=SQR(-1)',
        '180 X=LOG(0)',
        '190 X=10^40',
        '200 OPEN "I",#1,"B:NOFILE"',
        '210 X=1:X=X/0',
        '220 X=1:X=X\\0',
        '230 RESUME',
        '240 PRINT:PRINT "end";ERR;ERL:SYSTEM',
        '1000 PRINT ERR;ERL;:RESUME NEXT'),
    "TOK.BAS": prog(
        "10 PRINT A$;:IF X>=1 THEN 20 ELSE X=&H1F+1.5#+12345!+7%+&O7+300+65000:' r",
        '20 GOSUB 10:A=CVI(B$)+SQR(4)+FRE(0):FILES:REM x'),
    "DUMP.BAS": prog(
        '10 FOR F=1 TO 2:F$="B:T.BAS":IF F=2 THEN F$="B:TP.BAS"',
        '20 OPEN "R",#1,F$,1:FIELD #1,1 AS C$:N=LOF(1):PRINT F$;N',
        '30 FOR I=1 TO N:GET #1,I:PRINT RIGHT$("0"+HEX$(ASC(C$)),2);:NEXT:PRINT:CLOSE #1:NEXT'),
    "INPUT.BAS": prog(
        '10 INPUT "name";N$:PRINT "[";N$;"]"',
        '20 INPUT A,B:PRINT A+B',
        '30 INPUT "";C:PRINT C',
        '40 LINE INPUT "line? ";L$:PRINT L$',
        '50 INPUT X$,Y:PRINT X$;Y',
        '60 SYSTEM'),
    "SCR.BAS": prog(
        '10 KEY OFF:CLS',
        '20 LOCATE 5,10:PRINT "at 5,10";',
        '30 R=CSRLIN:C=POS(0):LOCATE 1,1:PRINT R;C',
        '40 PRINT CHR$(SCREEN(5,10));SCREEN(5,10);SCREEN(5,10,1)',
        '50 LOCATE 7,1:PRINT STRING$(85,"=")',
        '60 PRINT TAB(75);"wrap test"',
        '70 LOCATE 11,1:PRINT "a";:PRINT SPC(5);"b";TAB(3);"c"',
        '80 KEY 1,"hello"+CHR$(13):KEY LIST',
        '90 SYSTEM'),
    "CHAIN.BAS": prog(
        '10 OPEN "O",#1,"B:PART2.BAS"',
        '20 Q$=CHR$(34):PRINT #1,"10 PRINT ";Q$;"in part2";Q$;";A;B$;C"',
        '30 PRINT #1,"20 SYSTEM"',
        '40 CLOSE',
        '50 OPEN "O",#1,"B:SUB.BAS":PRINT #1,"1000 PRINT ";Q$;"merged";Q$:PRINT #1,"1010 RETURN":CLOSE',
        '60 CHAIN MERGE "B:SUB",70',
        '70 GOSUB 1000',
        '80 COMMON A,B$:A=42:B$="kept":C=1',
        '90 CHAIN "B:PART2"'),
    "FMODE.BAS": prog(
        '10 ON ERROR GOTO 900',
        '20 OPEN "B:F.TXT" FOR OUTPUT AS #1',
        '30 PRINT #1,"abc";TAB(6);"d";:PRINT #1,1,-2:PRINT #1,USING "##.#";3.14159:WRITE #1,"q",1.5,-2',
        '40 PRINT LOC(1);LOF(1):CLOSE #1',
        '50 OPEN "B:F.TXT" FOR APPEND AS #1:PRINT #1,"more":CLOSE',
        '60 OPEN "B:F.TXT" FOR INPUT AS #2',
        '70 PRINT INPUT$(5,#2);"|";LOC(2)',
        '80 WHILE NOT EOF(2):LINE INPUT #2,L$:PRINT "[";L$;"]":WEND',
        '90 INPUT #2,Z',
        '100 OPEN "B:F.TXT" FOR INPUT AS #2',
        '110 GET #2,1',
        '120 CLOSE:OPEN "R",#1,"B:R.DAT",10:FIELD #1,6 AS A$,6 AS B$',
        '130 PUT #1,0',
        '140 OPEN "B:BAD*.X" FOR OUTPUT AS #3',
        '150 OPEN "O",#4,"B:X"',
        '160 KILL "B:NOPE"',
        '170 NAME "B:F.TXT" AS "B:R.DAT"',
        '180 CLOSE:SYSTEM',
        '900 PRINT "err";ERR;"in";ERL:RESUME NEXT'),
    "VARS.BAS": prog(
        '10 DEFINT I-K:DEFSTR S:DEFDBL D',
        '20 I=3.7:S="str":D=1/3:PRINT I;S;D',
        '30 ABCDEFGH=1:ABCDEFGX=2:PRINT ABCDEFGH;ABCDEFGX',
        '40 A=1:A%=2:A!=3:A#=4:A$="5":PRINT A;A%;A!;A#;A$',
        '50 OPTION BASE 1:DIM M(2,3):FOR R=1 TO 2:FOR C=1 TO 3:M(R,C)=R*10+C:NEXT C,R:PRINT M(2,3);M(1,1)',
        '60 ERASE M:DIM M(5):PRINT M(5)',
        '70 X(10)=7:PRINT X(10)',
        '80 DIM Z%(2,2,2):Z%(2,2,2)=-1:PRINT Z%(2,2,2)',
        '90 DEF FNA$(X$)=X$+"!":PRINT FNA$("hi")',
        '100 DEF FNB(X,Y)=X*Y+A:PRINT FNB(2,3)',
        '110 PRINT VARPTR(A)-VARPTR(A%);VARPTR(A#)-VARPTR(A$)',
        '120 SWAP A,A!:PRINT A;A!',
        '130 L=0:FOR I=1 TO 3:FOR J=I TO 3:L=L+1:NEXT J,I:PRINT L',
        '140 FOR Q=1 TO 0:PRINT "never":NEXT:PRINT Q',
        '150 FOR Q=1 TO 2 STEP .5:PRINT Q;:NEXT:PRINT',
        '160 PRINT FRE(0)',
        '170 SYSTEM'),
    "MEM.BAS": prog(
        '10 PRINT PEEK(&H30)+256*PEEK(&H31);PEEK(&H358)+256*PEEK(&H359)',
        '20 A=1:B$="xy":DIM C(3)',
        '30 PRINT VARPTR(A);VARPTR(B$);VARPTR(C(0));PEEK(VARPTR(B$));FRE(0)',
        '40 P=PEEK(&H30)+256*PEEK(&H31):FOR I=P-1 TO P+22:PRINT HEX$(PEEK(I));" ";:NEXT:PRINT',
        '50 CLEAR ,40000:PRINT FRE(0)',
        '60 SYSTEM'),
}
B = "b:label:BASIC"
CASES = [
    ("b01_print", "PRINT.BAS", "basic print", []),
    ("b02_loops", "LOOPS.BAS", "basic loops", []),
    ("b03_math", "MATH.BAS", "basic math", []),
    ("b04_sequential", "SEQ.BAS," + B, "basic seq", [("$A>", "type b:seq.dat\r")]),
    ("b05_random", "RAND.BAS," + B, "basic rand", []),
    ("b06_errors", "ERRS.BAS," + B, "basic errs", []),
    ("b07_files", "FILES.BAS," + B, "basic files", []),
    ("b08_interactive", B, "basic",
     [("Ok", "key off\r"), ("$Ok", '10 PRINT "hi";2*3\r'), ("$", "20 GOTO 40\r"), ("$", '30 PRINT "skipped"\r'),
      ("$", '40 PRINT "end"\r'), ("$", "run\r"), ("$Ok", "list\r"), ("$Ok", 'save "b:p"\r'),
      ("$Ok", 'save "b:pa",a\r'), ("$Ok", "new\r"), ("$Ok", 'load "b:p"\r'), ("$Ok", "list 20-\r"),
      ("$Ok", "system\r"), ("$A>", "type b:pa.bas\r")]),
    ("b09_direct_errors", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "print 1/0\r"), ("$Ok", "goto 100\r"), ("$Ok", 'x$="a"+1\r'), ("$Ok", "foo bar\r"),
      ("$Ok", "system\r")]),
    ("b10_numfmt", "NUMFMT.BAS", "basic numfmt", []),
    ("b11_using", "USING.BAS", "basic using", []),
    ("b12_strings", "STRS.BAS", "basic strs", []),
    ("b13_trap", "TRAP.BAS," + B, "basic trap", [("$Ok", "system\r")]),
    ("b14_errmsg1", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r")] + [("$Ok", "error %d\r" % n) for n in (15, 16, 17, 18, 19, 20, 22)]
     + [("$Ok", "system\r")]),
    ("b15_errmsg2", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r")] + [("$Ok", "error %d\r" % n) for n in (23, 24, 25, 26, 27, 29, 21)]
     + [("$Ok", "system\r")]),
    ("b16_errmsg3", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r")] + [("$Ok", "error %d\r" % n) for n in (50, 51, 52, 54, 55, 57, 58)]
     + [("$Ok", "system\r")]),
    ("b17_errmsg4", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r")] + [("$Ok", "error %d\r" % n) for n in (61, 62, 63, 64, 66, 67, 70)]
     + [("$Ok", "system\r")]),
    ("b18_errmsg5", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r")] + [("$Ok", "error %d\r" % n) for n in (71, 72, 74, 75, 76, 255)]
     + [("$Ok", "play \"c\"\r"), ("$Ok", "system\r")]),
    ("b19_tokens", "TOK.BAS,DUMP.BAS," + B, "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r"), ("$Ok", 'load "tok"\r'), ("$Ok", 'save "b:t"\r'), ("$Ok", 'save "b:tp",p\r'),
      ("$Ok", 'load "dump"\r'), ("$Ok", "run\r"), ("$Ok", 'load "b:tp"\r'), ("$Ok", "list\r"), ("$Ok", "system\r")]),
    ("b20_detok", "TOK.BAS", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r"), ("$Ok", 'load "tok"\r'), ("$Ok", "30 for i=1to10 step 3:? i;:next\r"),
      ("$", "40 a = b+c  :   rem  spaces\r"), ("$", "50 x=1e5:y=.5e-3:z#=1.234567890123d2:w=123456789\r"),
      ("$", "list\r"), ("$Ok", "system\r")]),
    ("b21_editor", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", '^L10 print "abcd^H^Hxy"\r'), ("$", "^K2\r"),
      ("$", "40 int 5^B^B^Rpr\r"), ("$", "50 print 6^B^B^B^N;7\r"), ("$", "60 print 8^C"),
      ("$", '70 print^J"x"\r'), ("$", "80 print 1^B^B^E\r"), ("$", "list\r"), ("$Ok", "run\r"),
      ("$Ok", "system\r")]),
    ("b22_commands", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r"), ("$Ok", "auto\r"), ("$", 'print "a"\r'), ("$", 'print "b":stop\r'),
      ("$", 'print "c"\r'), ("$", "^C"), ("$Ok", "renum 100,,5\r"), ("$Ok", "tron\r"), ("$Ok", "run\r"),
      ("$Ok", "cont\r"), ("$Ok", "troff\r"), ("$Ok", "delete 110\r"), ("$Ok", "list\r"),
      ("$Ok", "x=5:print x*2\r"), ("$Ok", "system\r")]),
    ("b23_input", "INPUT.BAS", "basic input",
     [("$?", "bob\r"), ("$?", "1\r"), ("$?", "1,2\r"), ("$?", "abc\r"), ("$?", "7\r"), ("$line?", 'a, "b", c\r'),
      ("$?", '"q,r",1,2\r'), ("$?", '"q,r",1\r')]),
    ("b24_screen", "SCR.BAS", "basic scr", []),
    ("b25_chain", "CHAIN.BAS," + B, "basic chain", []),
    ("b26_filemodes", "FMODE.BAS," + B, "basic fmode", [("$A>", "type b:f.txt\r")]),
    ("b27_vars", "VARS.BAS", "basic vars", []),
    ("b28_memory", "MEM.BAS", "basic mem", []),
    ("b29_keyline", "-", "basic", [("Ok", 'key 2,"ab"\rprint 1\r')]),
    ("b30_width40", "-", "basic", [("Ok", "width 40\rprint 1\r")]),
    ("b31_errmsg6", "-", "basic",
     [("Ok", "key off\r"), ("$Ok", "cls\r")] + [("$Ok", "error %d\r" % n) for n in (28, 56, 59, 60, 65, 68, 69)]
     + [("$Ok", "system\r")]),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, data in FILES.items():
        (out / name).write_bytes(data)
    for name, files, cmd, pairs in CASES:
        print("\t".join([name, files, cmd] + [x for p in pairs for x in p]))


if __name__ == "__main__":
    main()
