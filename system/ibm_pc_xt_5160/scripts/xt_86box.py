#!/usr/bin/env python3
"""Boot PC DOS 2.00 on 86Box's IBM PC XT 5160 with a text printer on LPT1; print the screen and
what was printed.

    xt_86box.py <86Box> <xt86 roms dir> <drive A image> <drive B image or -> <timeout s> \
        [<text to wait for> <keys to type>]...

86Box (built with -D AUTOMATION=ON; see ../../ibm_ps2_model30/toolchain/spec/audit.md for the
commit) is driven through its automation socket (doc/specifications/automation.md): the machine
starts held, so emulated time passes only when this script asks for it. Machine: `ibmxt` with
BIOS `ibm5160_1501512_5000027` (BIOS_5160_08NOV82_U18_1501512.BIN and
BIOS_5160_08NOV82_U19_5000027.BIN from <xt86 roms dir>: the 1982-11-08 BIOS and Cassette BASIC
C1.10, the same bytes as MartyPC's), 8088 at 4.77 MHz, 256 KiB, CGA, two 5.25-inch 360 KiB
drives, LPT1 the Generic Text Printer (`text_prt`), the clock not synchronised with the host.

Waits as in martypc_oracle: a text is waited for anywhere on the screen; a text starting with `$`
waits for the last non-blank row to end with the rest, once the screen differs from the one at
the previous match. Keys: `\\r` is Enter, `^X` Ctrl+X, capitals and shifted characters with
Shift. After the last pair the machine runs until the screen has not changed for 4 s of emulated
time (and 6 s more, so the printer's page timeout writes the page), or the timeout. The 25 rows
are printed as `|row`, then each printed line as `#lpt|line` (pages separated by `#lpt|<FF>`).
Exit status 1 if a wait timed out. Standard library only.
"""
import glob
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time

# Scan code set 1 make codes, US layout.
BASE = {c: v for c, v in zip("1234567890", range(0x02, 0x0C))}
BASE.update({c: v for c, v in zip("qwertyuiop", range(0x10, 0x1A))})
BASE.update({c: v for c, v in zip("asdfghjkl", range(0x1E, 0x27))})
BASE.update({c: v for c, v in zip("zxcvbnm", range(0x2C, 0x33))})
BASE.update({"-": 0x0C, "=": 0x0D, "[": 0x1A, "]": 0x1B, ";": 0x27, "'": 0x28, "`": 0x29,
             "\\": 0x2B, ",": 0x33, ".": 0x34, "/": 0x35, " ": 0x39, "\r": 0x1C})
SHIFTED = dict(zip('!@#$%^&*()_+{}:"~|<>?', "1234567890-=[];'`\\,./"))
SHIFT, CTRL = 0x2A, 0x1D

CONFIG = """[General]
confirm_exit = 0
confirm_reset = 0
confirm_save = 0

[Machine]
machine = ibmxt
cpu_family = 8088
cpu_speed = 4772728
cpu_multi = 1
cpu_use_dynarec = 0
fpu_type = none
mem_size = 256
time_sync = disabled

[IBM PC/XT (1982)]
bios = ibm5160_1501512_5000027

[Video]
gfxcard = cga

[Sound]
sndcard = none

[Ports (COM & LPT)]
lpt1_enabled = 1
lpt1_device = text_prt

[Floppy and CD-ROM drives]
fdd_01_type = 525_2dd
fdd_01_fn = {a}
fdd_01_check_bpb = 0
fdd_02_type = 525_2dd
fdd_02_fn = {b}
fdd_02_check_bpb = 0
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
    return [bytes.fromhex(r).decode("latin-1").rstrip() for r in t["chars"]]


def keys_of(text):
    """(code, modifier) per key; `^X` is Ctrl+X."""
    out, i = [], 0
    while i < len(text):
        c = text[i]
        if c == "^" and i + 1 < len(text) and text[i + 1].isalpha():
            out.append((BASE[text[i + 1].lower()], CTRL))
            i += 2
            continue
        if c.isupper():
            out.append((BASE[c.lower()], SHIFT))
        elif c in SHIFTED:
            out.append((BASE[SHIFTED[c]], SHIFT))
        elif c in BASE:
            out.append((BASE[c], None))
        else:
            raise SystemExit(f"no key for {c!r}")
        i += 1
    return out


def last_row(rows):
    for r in reversed(rows):
        if r.strip():
            return r.rstrip()
    return ""


def main():
    a = sys.argv
    if len(a) < 6 or (len(a) - 6) % 2:
        print(__doc__, file=sys.stderr)
        return 2
    exe, roms = os.path.abspath(a[1]), os.path.abspath(a[2])
    image_a, image_b, timeout = os.path.abspath(a[3]), a[4], float(a[5])
    script = [(a[i], a[i + 1].replace("\\r", "\r")) for i in range(6, len(a), 2)]
    work = tempfile.mkdtemp(prefix="xt_86box_")
    try:
        rom_dir = os.path.join(work, "roms", "machines", "ibmxt")
        os.makedirs(rom_dir)
        for chip in ("BIOS_5160_08NOV82_U18_1501512.BIN", "BIOS_5160_08NOV82_U19_5000027.BIN"):
            shutil.copy(os.path.join(roms, chip), rom_dir)
        vm = os.path.join(work, "vm")
        os.makedirs(vm)
        disk_a = os.path.join(vm, "a.img")
        shutil.copy(image_a, disk_a)
        disk_b = ""
        if image_b != "-":
            disk_b = os.path.join(vm, "b.img")
            shutil.copy(os.path.abspath(image_b), disk_b)
        with open(os.path.join(vm, "86box.cfg"), "w") as f:
            f.write(CONFIG.replace("{a}", disk_a).replace("{b}", disk_b))
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
            matched_at = None
            for text, keys in script:
                if text.startswith("$"):
                    end, spent, ok = text[1:], 0, False
                    while spent < ms:
                        rows = screen(box)
                        if rows != matched_at and last_row(rows).endswith(end):
                            ok = True
                            break
                        box("step 50")
                        spent += 50
                else:
                    ok = bool(box(f"wait text {ms} {text}").get("matched"))
                    rows = screen(box)
                if not ok:
                    print(f"timed out waiting for {text!r}", file=sys.stderr)
                    failed = True
                    break
                matched_at = rows
                box("step 400")  # let the program reach its keyboard read
                for code, mod in keys_of(keys):
                    if mod:
                        box(f"key down {mod:#x}")
                    box(f"key down {code:#x}")
                    box("step 30")
                    box(f"key up {code:#x}")
                    if mod:
                        box(f"key up {mod:#x}")
                    box("step 30")
            # Settle: 4 s without a change, then 6 s for the printer's page timeout.
            last, still, spent = None, 0, 0
            while spent < ms and still < 4000:
                box("step 250")
                spent += 250
                now = screen(box)
                still = still + 250 if now == last else 0
                last = now
            box("step 6000")
            for row in screen(box):
                print("|" + row)
            box("quit")
        finally:
            try:
                proc.wait(timeout=20)
            except subprocess.TimeoutExpired:
                proc.kill()
        pages = sorted(glob.glob(os.path.join(vm, "printer", "*.txt")))
        for p in pages:
            with open(p, "rb") as f:
                for line in f.read().decode("latin-1").split("\n"):
                    print("#lpt|" + line.replace("\x0c", "<FF>").rstrip("\r"))
        return 1 if failed else 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
