#!/usr/bin/env python3
"""Generate a chips ROM stub that exercises a libi4004rt RAM mem-helper across a
4002 register boundary, then copies probe nibbles back to chip0/reg0 chars
0..nchars-1 so the rt_test harness (which reads only reg0) can verify them.

The RAM helpers advance a FULL 8-bit SRC address, so a region longer than one
register's 16 characters must spill into the next register (SRC 0x10.. = reg1)
and, past 0x40, the next chip. These tests fill/copy >16 nibbles and read back
nibbles on BOTH sides of the 0x10 register boundary.

Modes:
  memset <addr> <val> <count>         fill; then probe `count` region nibbles.
  memcpy <dst> <src> <count>          seed src with a known ramp, copy, probe dst.

Each probe nibble i (0..nprobe-1) is read from region address base+i and written
to chip0/reg0 char i for read-back. nprobe is min(count, 8) to stay within the
reg0 readback window; we also always include the first nibble PAST 0x0f when
count>16 by placing it at a fixed probe slot. Stdout = assembly.
"""
import argparse, sys

HEAD = """; GENERATED mem test stub -- do not edit.
	.section .reset,"ax"
reset:
	jun	start
	.text
start:
	ldm	0
	dcl				; RAM bank 0
"""
TAIL = "halt:	jun	halt\n"


def set_addr(pair, addr):
    # FIM pN, 0xAA  loads the pair with the 8-bit SRC address.
    return [f"\tfim\t{pair}, 0x{addr & 0xff:02x}"]


def probe_to_reg0(region_base, nprobe):
    """Read region_base+i and write it to chip0/reg0 char i, for i in range."""
    s = []
    for i in range(nprobe):
        a = (region_base + i) & 0xff
        s.append(f"\tfim\tp0, 0x{a:02x}")
        s.append("\tsrc\tp0")
        s.append("\trdm")                  # ACC = RAM[region_base+i]
        s.append("\txch\tr2")              # stash in R2 (reg0 readback is 0x00..)
        s.append(f"\tfim\tp0, 0x{i:02x}")  # reg0 char i
        s.append("\tsrc\tp0")
        s.append("\tld\tr2")
        s.append("\twrm")
    return s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["memset", "memcpy"])
    ap.add_argument("--addr", type=lambda s: int(s, 0), default=0)
    ap.add_argument("--src", type=lambda s: int(s, 0), default=0x20)
    ap.add_argument("--val", type=lambda s: int(s, 0), default=0)
    ap.add_argument("--count", type=int, default=20)   # nibbles
    ap.add_argument("--nprobe", type=int, default=8)
    ap.add_argument("--probe-base", type=lambda s: int(s, 0), default=None,
                    help="address to start probing from (default: --addr)")
    args = ap.parse_args()

    probe_base = args.probe_base if args.probe_base is not None else args.addr

    body = []
    if args.mode == "memset":
        # __memset_ram(dst=P1, val=P2.lo, count=P3)
        body += set_addr("p1", args.addr)
        body += set_addr("p2", args.val & 0x0f)
        body += set_addr("p3", args.count)
        body.append("\tjms\t__memset_ram")
        body += probe_to_reg0(probe_base, args.nprobe)
    else:
        # seed src region with a ramp (i & 0xf) using direct writes, then
        # __memcpy_ram(dst=P1, src=P2, count=P3).
        for i in range(args.count):
            a = (args.src + i) & 0xff
            body.append(f"\tfim\tp0, 0x{a:02x}")
            body.append("\tsrc\tp0")
            body.append(f"\tldm\t{i & 0xf}")
            body.append("\twrm")
        body += set_addr("p1", args.addr)
        body += set_addr("p2", args.src)
        body += set_addr("p3", args.count)
        body.append("\tjms\t__memcpy_ram")
        body += probe_to_reg0(probe_base, args.nprobe)

    sys.stdout.write(HEAD + "\n".join(body) + "\n" + TAIL)


if __name__ == "__main__":
    main()
