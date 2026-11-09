#!/usr/bin/env python3
"""decode.py <printed bytes> [--lines N] [--pbm out.pbm]

Decodes what was sent to an IBM Graphics Printer (Epson MX-80 command set as far as GRAPHICS and
the BIOS Print Screen use it): ESC 3 n (line spacing n/216 inch), ESC L n1 n2 (n1 + 256 n2 dot
columns of dual-density bit image follow, bit 7 the top pin), CR, LF, FF; other bytes are text.
Prints one line per printer line (up to --lines, default all): its bit image's column count
and how many columns hold a dot, or its text, and the control bytes, then a summary.
--pbm writes the bit-image lines as a portable bitmap, 8 dots a line (the line spacing is not
applied: GRAPHICS's 24/216 inch is exactly 8 pins), text lines left out."""
import sys


def decode(data):
    i, line, out, rows = 0, [], [], []
    image = None

    def end(what):
        nonlocal line, image
        out.append((what, line, image))
        if image is not None:
            rows.append(image)
        line, image = [], None

    while i < len(data):
        b = data[i]
        if b == 0x1B and i + 1 < len(data):
            c = data[i + 1]
            if c == ord("3"):
                line.append(f"ESC 3 {data[i + 2]}")
                i += 3
                continue
            if c == ord("L"):
                n = data[i + 2] | data[i + 3] << 8
                image = (image or b"") + data[i + 4:i + 4 + n]
                line.append(f"ESC L {n}")
                i += 4 + n
                continue
            line.append(f"ESC {chr(c) if 32 < c < 127 else hex(c)}")
            i += 2
            continue
        if b == 0x0A:
            line.append("LF")
            end("LF")
        elif b == 0x0D:
            line.append("CR")
        elif b == 0x0C:
            line.append("FF")
            end("FF")
        else:
            if line and isinstance(line[-1], bytearray):
                line[-1].append(b)
            else:
                line.append(bytearray([b]))
        i += 1
    if line:
        end("end")
    return out, rows


def show(line, image):
    parts = []
    for p in line:
        if isinstance(p, bytearray):
            parts.append(repr(bytes(p))[1:])
        else:
            parts.append(p)
    s = " ".join(parts)
    if image is not None:
        s += f"  [{len(image)} columns, {sum(1 for x in image if x)} with dots]"
    return s


def main():
    args = sys.argv[1:]
    data = open(args[0], "rb").read()
    limit = int(args[args.index("--lines") + 1]) if "--lines" in args else None
    out, rows = decode(data)
    for n, (_, line, image) in enumerate(out[:limit]):
        print(f"{n + 1:3}: {show(line, image)}")
    print(f"{len(data)} bytes, {len(out)} lines, {len(rows)} with a bit image")
    if "--pbm" in args:
        w = max((len(r) for r in rows), default=0)
        h = 8 * len(rows)
        bits = [[0] * w for _ in range(h)]
        for n, r in enumerate(rows):
            for x, col in enumerate(r):
                for pin in range(8):
                    if col & (0x80 >> pin):
                        bits[8 * n + pin][x] = 1
        with open(args[args.index("--pbm") + 1], "w") as f:
            f.write(f"P1\n{w} {h}\n")
            for row in bits:
                f.write("".join(map(str, row)) + "\n")


if __name__ == "__main__":
    main()
