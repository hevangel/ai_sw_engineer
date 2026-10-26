#!/usr/bin/env python3
r"""Capture SCELBAL oracle transcripts from the ORIGINAL program.

    capture_oracle.py [original.bin]   (default build/original.bin)

Runs build/original.bin (scripts/build_original.py: the 1974-75 listing with
the Micral N teletype) on the ai_hw_engineer RTL harness for each scenario
below and writes oracle/transcripts/NN_name.txt: `<` lines are the keys
typed, `>` lines the printed bytes (\r \n \ \xNN escapes), split after each
LF. A session ends when SCELBAL waits for a key and none is left.

Every session starts with SCR: the listing leaves the end-of-program
pointer zero until the first SCR (spec BAS-2).
"""
import hashlib
import os
import pathlib
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TITLE = os.path.dirname(HERE)
RTL = os.environ.get("MICRAL_SIM", os.path.join(TITLE, "..", "platform", "sim", "build", "micral_rtl"))


def contained(base, *parts):
    """Resolve parts under base and refuse paths that escape it (SAST guard)."""
    real_base = pathlib.Path(base).resolve()
    p = real_base.joinpath(*parts).resolve()
    if p != real_base and real_base not in p.parents:
        raise SystemExit(f"path escapes {real_base}: {parts}")
    return p


def lines(*ls):
    return "".join(l + "\r" for l in ls)


SCENARIOS = [
    ("01_print_numbers", lines(
        "SCR",
        "PRINT 1,0,-1,2,10,100",
        "PRINT 1/3,2/3,10/3,-1/3",
        "PRINT 123456,1234567,8388607,8388608,12345678",
        "PRINT 1E6,1E7,1E10,1.5E20,-2.5E-3",
        "PRINT .1,.01,.001,.0001,1.5E-5",
        "PRINT 3.14159;2.71828;-.5;.5;1.25",
        "PRINT 7*7,2^10,2^-2,5-3*2,(5-3)*2",
        "PRINT -3^2,2*-3,-(2+3),10/4/5,2^3^2",
        "PRINT 1<2;2<1;1=1;1<>2;3>=3;3<=2;2>1;2=>2;1=<0;2><2",
        "PRINT 0.5+0.25,1-1E-7,99999*99999,1E30*1E30",
        "PRINT",
        "PRINT 1;",
        "PRINT 2,",
        "PRINT 3")),
    ("02_program_edit", lines(
        "SCR",
        "30 PRINT \"THIRD\"",
        "10 PRINT \"FIRST\"",
        "20 PRINT 'SECOND'",
        "25 PRINT \"EXTRA\"",
        "LIST",
        "25",
        "20 PRINT \"NEW SECOND\";20",
        "LIST",
        "RUN",
        "40 END",
        "35 PRINT \"NOT REACHED AFTER END?\"",
        "RUN",
        "SAVE",
        "LOAD",
        "LIST")),
    ("03_functions", lines(
        "SCR",
        "PRINT INT(3.7),INT(-3.7),INT(5),INT(-5),INT(.5)",
        "PRINT SGN(-2),SGN(0),SGN(3.5),ABS(-4.25),ABS(6)",
        "PRINT SQR(2),SQR(16),SQR(.25),SQR(1E6),SQR(3)",
        "PRINT RND(1),RND(1),RND(1)",
        "PRINT \"A\";TAB(10);\"B\";TAB(5);\"C\"",
        "PRINT CHR(72);CHR(73);CHR(33)",
        "10 FOR I=1 TO 5",
        "20 PRINT TAB(I*3);I",
        "30 NEXT I",
        "RUN",
        "PRINT INT(RND(1)*100),INT(RND(1)*100)")),
    ("04_loops_subroutines", lines(
        "SCR",
        "10 FOR I=1 TO 3",
        "20 FOR J=10 TO 1 STEP -4.5",
        "30 PRINT I;J,",
        "40 NEXT J",
        "50 GOSUB 100",
        "60 NEXT I",
        "70 PRINT \"DONE\";I;J",
        "80 END",
        "100 PRINT \"SUB\";I*I",
        "110 IF I=2 THEN 130",
        "120 RETURN",
        "130 PRINT \"TWO\"",
        "140 RETURN",
        "RUN",
        "10 FOR X=0 TO 1 STEP .25",
        "20 PRINT X;",
        "30 NEXT X",
        "35 PRINT",
        "40 END",
        "RUN",
        "GOSUB 200",
        "200 PRINT \"DIRECT GOSUB\"",
        "210 RETURN",
        "GOSUB 200",
        "IF 1 THEN PRINT \"YES\"",
        "IF 0 THEN PRINT \"NO\"",
        "IF 2>1 GOTO 200")),
    ("05_input", lines(
        "SCR",
        "10 INPUT A,B",
        "20 PRINT \"SUM\";A+B",
        "30 INPUT C$",
        "40 PRINT C,C-128",
        "50 IF A=0 THEN 70",
        "60 GOTO 10",
        "70 END",
        "RUN",
        "3",
        "4.5",
        "Z",
        "-1E2",
        ".25",
        "!",
        "0",
        "0",
        "A")),
    ("06_errors", lines(
        "SCR",
        "PRINT 1/0",
        "PRINT SQR(-1)",
        "PRINT (1+2",
        "PRINT 1+2)",
        "PRINT \"OPEN",
        "PRINT 2**3",
        "FOO",
        "10 GOTO 99",
        "RUN",
        "10 RETURN",
        "RUN",
        "10 NEXT I",
        "RUN",
        "10 FOR I=1 2",
        "RUN",
        "10 IF 1 PRINT 2",
        "RUN",
        "10 LET X 5",
        "RUN",
        "10 X=Q(3)",
        "RUN",
        "10 INPUT N",
        "RUN",
        "1X",
        "10 DIM A3)",
        "RUN",
        "SCR",
        "PRINT 1E50*1E50*1E50*1E50",
        "PRINT INT(1E10)",
        "PRINT 12345678901234567890",
        "PRINT \"" + "X" * 70 + "\"",
        "PRINT 1\x7f\x7f2+3",
        "PRINT 4\x03",
        "PRINT 5")),
    ("07_arrays", lines(
        "SCR",
        "10 DIM A(10),B(5)",
        "20 FOR I=1 TO 10",
        "30 A(I)=I*I",
        "40 NEXT I",
        "50 FOR I=1 TO 5",
        "60 B(I)=A(I)+A(I+5)",
        "70 PRINT I;A(I);B(I)",
        "80 NEXT I",
        "90 PRINT A(10)/B(5)",
        "100 INPUT A(3)",
        "110 PRINT A(3)",
        "120 PRINT A(-1)",
        "RUN",
        "7.5")),
    ("08_primes", lines(
        "SCR",
        "10 REM PRIMES BELOW 60",
        "20 FOR N=2 TO 60",
        "30 FOR D=2 TO SQR(N)",
        "40 IF N/D=INT(N/D) THEN 70",
        "50 NEXT D",
        "60 PRINT N;",
        "70 NEXT N",
        "80 PRINT",
        "90 PRINT \"COUNTED\"",
        "RUN")),
    ("09_program", lines(
        "SCR",
        "10 REM SQUARES, SUMS, FACTORIAL, ODD PRIMES",
        "20 PRINT \"N\",\"SQUARE\",\"CUBE\"",
        "30 FOR N=1 TO 6",
        "40 PRINT N,N*N,N*N*N",
        "50 NEXT N",
        "60 LET S=0",
        "70 FOR K=1 TO 100",
        "80 S=S+K",
        "90 NEXT K",
        "100 PRINT \"SUM 1..100 =\";S",
        "110 GOSUB 200",
        "120 PRINT \"10 FACTORIAL\";F",
        "130 FOR N=3 TO 41 STEP 2",
        "140 P=1",
        "150 FOR D=3 TO N-2 STEP 2",
        "160 IF N/D=INT(N/D) THEN P=0",
        "170 NEXT D",
        "180 IF P=1 THEN PRINT N;",
        "190 NEXT N",
        "195 END",
        "200 F=1",
        "210 FOR K=2 TO 10",
        "220 F=F*K",
        "230 NEXT K",
        "240 RETURN",
        "RUN",
        "PRINT",
        "LIST")),
    ("10_limits", lines(
        "SCR",
        "LIST",
        "RUN",
        "PRINT X;Y",
        "AB=5",
        "A1=2",
        "PRINT AB*A1,AB+A1,AB-A1,AB/A1",
        "LET C=AB/A1",
        "PRINT C;'SINGLE';\"DOUBLE\"",
        "D=4", "E=5", "F=6", "G=7", "H=8", "I=9", "J=10", "K=11", "L=12",
        "M=13", "N=14", "O=15", "P=16", "Q=17", "R=18",
        "S=19",
        "PRINT AB+A1+C+D+E+F+G+H+I+J+K+L+M+N+O+P+Q+R",
        "PRINT \"" + "Y" * 76 + "\"",
        "PRINT 1",
        "10 PRINT \"ONE\"",
        "10",
        "LIST",
        "10 PRINT \"TEN\"",
        "5 PRINT \"FIVE\"",
        "7 GOTO 10",
        "RUN")),
]


def esc(b):
    out = []
    for c in b:
        if c == 13:
            out.append(r"\r")
        elif c == 10:
            out.append(r"\n")
        elif c == 92:
            out.append("\\\\")
        elif 32 <= c < 127:
            out.append(chr(c))
        else:
            out.append(r"\x%02x" % c)
    return "".join(out)


def main():
    orig = sys.argv[1] if len(sys.argv) > 1 else os.path.join(TITLE, "build", "original.bin")
    sha = hashlib.sha256(open(orig, "rb").read()).hexdigest()
    only = sys.argv[2:]
    for name, keys in SCENARIOS:
        if only and name not in only:
            continue
        with tempfile.TemporaryDirectory() as d:
            tin, tout = contained(d, "in"), contained(d, "out")
            tin.write_bytes(keys.encode("latin-1"))
            r = subprocess.run([RTL, "--ram", f"{orig}@0", "--tty-in", tin, "--tty-out", tout,
                                "--idle", "20000", "--max-cycles", "2000000000", "--regs"],
                               capture_output=True, text=True)
            out = tout.read_bytes()
        if "idle" not in r.stderr:
            raise SystemExit(f"{name}: the original did not finish waiting for a key: {r.stderr}")
        text = [f"# Captured from the original (build/original.bin, SHA-256 {sha}) on the",
                "# ai_hw_engineer 8008 RTL by scripts/capture_oracle.py. Do not edit."]
        text += ["< " + esc(l.encode("latin-1") + b"\r") for l in keys.split("\r")[:-1]]
        cur = bytearray()
        for c in out:
            cur.append(c)
            if c == 10:
                text.append("> " + esc(cur))
                cur = bytearray()
        if cur:
            text.append("> " + esc(cur))
        p = contained(TITLE, "oracle", "transcripts", name + ".txt")
        p.write_text("\n".join(text) + "\n", newline="\n")
        print(f"{name}: {len(out)} bytes, {r.stderr.strip()}")


if __name__ == "__main__":
    main()
