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
not come back to BASIC within the case (to look into with the rewrite)."""
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
