#!/usr/bin/env python3
"""Boot a 720 KiB diskette image on 86Box's IBM PS/2 Model 30 and print the text screen.

    ps2_86box.py <86Box> <ps2 roms dir> <image> <timeout s> [<text to wait for> <keys to type>]...

86Box (built with -D AUTOMATION=ON; see report/report.md for the commit) is driven through its
automation socket (doc/specifications/automation.md): the machine starts held, so emulated time
passes only when this script asks for it. Machine: `ibmps2_m30` with BIOS revision 0
(`ibmps2_m30_rev0`: 68X1687 + 68X1627 from <ps2 roms dir>), 8086, 640 KiB, MCGA, drive A a 3.5-inch
720 KiB drive holding <image>, no drive B, the clock not synchronised with the host.

Each pair of arguments after the timeout: run until <text> is on the screen, then type <keys>
(`\\r` is Enter, `{F1}` the F1 key). After the last pair the machine runs until the screen has not changed for two
seconds of emulated time, or the timeout. The 25 rows are printed as `|row`, then the CPU registers
(`regs`) on stderr. Exit status 1 if a wait timed out. Standard library only.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time

# Scan code set 1 make codes (IBM keyboard), US layout, for what the scripts type.
SCAN = {c: v for c, v in zip("1234567890", range(0x02, 0x0C))}
SCAN.update({c: v for c, v in zip("qwertyuiop", range(0x10, 0x1A))})
SCAN.update({c: v for c, v in zip("asdfghjkl", range(0x1E, 0x27))})
SCAN.update({c: v for c, v in zip("zxcvbnm", range(0x2C, 0x33))})
SCAN.update({"-": 0x0C, ".": 0x34, " ": 0x39, "\r": 0x1C})

CONFIG = """[General]
confirm_exit = 0
confirm_reset = 0
confirm_save = 0

[Machine]
machine = ibmps2_m30
cpu_family = 8086
cpu_speed = 8000000
cpu_multi = 1
cpu_use_dynarec = 0
fpu_type = none
mem_size = 640
time_sync = disabled

[IBM PS/2 model 30 (8086)]
bios = ibmps2_m30_rev0

[Video]
gfxcard = internal

[Sound]
sndcard = none

[Floppy and CD-ROM drives]
fdd_01_type = 35_2dd
fdd_01_fn = {image}
fdd_01_check_bpb = 0
fdd_02_type = none
"""


class Box:
    def __init__(self, path):
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        for _ in range(300):
            try:
                self.s.connect(path)
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"86Box did not open {path}")
        self.f = self.s.makefile("rb")

    def __call__(self, line):
        self.s.sendall(line.encode("latin-1") + b"\n")
        reply = json.loads(self.f.readline())
        if not reply.get("ok", False):
            raise SystemExit(f"86Box: {line!r}: {reply}")
        return reply


def screen(box):
    t = box("text")
    return [bytes.fromhex(r).decode("cp437").rstrip() for r in t["chars"]]


def main():
    a = sys.argv
    if len(a) < 5 or (len(a) - 5) % 2:
        print(__doc__, file=sys.stderr)
        return 2
    exe, roms, image, timeout = os.path.abspath(a[1]), os.path.abspath(a[2]), os.path.abspath(a[3]), float(a[4])
    script = [(a[i], a[i + 1].replace("\\r", "\r")) for i in range(5, len(a), 2)]
    work = tempfile.mkdtemp(prefix="ps2_86box_")
    try:
        rom_dir = os.path.join(work, "roms", "machines", "ibmps2_m30")
        os.makedirs(rom_dir)
        for chip in ("68X1687.BIN", "68X1627.BIN"):
            shutil.copy(os.path.join(roms, chip), rom_dir)
        vm = os.path.join(work, "vm")
        os.makedirs(vm)
        disk = os.path.join(vm, "a.img")
        shutil.copy(image, disk)
        with open(os.path.join(vm, "86box.cfg"), "w") as f:
            f.write(CONFIG.replace("{image}", disk))
        sock = os.path.join(work, "auto.sock")
        env = {**os.environ, "SDL_VIDEODRIVER": "dummy", "SDL_AUDIODRIVER": "dummy"}
        proc = subprocess.Popen(
            [exe, "--vmpath", vm, "--rompath", os.path.join(work, "roms"), "--noconfirm",
             "--automation", "unix:" + sock, "--automation-hold", "--automation-seed", "1"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
        failed = False
        try:
            box = Box(sock)
            ms = int(timeout * 1000)
            for text, keys in script:
                if not box(f"wait text {ms} {text}").get("matched"):
                    print(f"timed out waiting for {text!r}", file=sys.stderr)
                    failed = True
                    break
                box("step 500")  # let the program reach its keyboard wait
                codes = []
                rest = keys
                while rest:
                    if rest.startswith("{F1}"):
                        codes.append(0x3B)
                        rest = rest[4:]
                        continue
                    code = SCAN.get(rest[0].lower())
                    if code is None:
                        raise SystemExit(f"no key for {rest[0]!r}")
                    codes.append(code)
                    rest = rest[1:]
                for code in codes:
                    box(f"key down {code:#x}")
                    box(f"key up {code:#x}")
            # Settle: run until two seconds pass without a change, within the timeout.
            last, still, spent = None, 0, 0
            while spent < ms and still < 2000:
                box("step 250")
                spent += 250
                now = screen(box)
                still = still + 250 if now == last else 0
                last = now
            for row in screen(box):
                print("|" + row)
            regs = box("regs")
            print({k: regs.get(k) for k in ("cs", "eip", "eax", "esp")}, file=sys.stderr)
            box("quit")
        finally:
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
        return 1 if failed else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
