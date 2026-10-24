// Micral N board model around an 8008 core: memory with the boot-ROM switch,
// the teletype, the stack card and the floppy subsystem as io_map.md
// describes them. Used by the Verilator harness (rtl_main.cpp); the CPU is the
// ai_hw_engineer RTL, everything else is this file.
#pragma once
#include <cstdint>
#include <deque>
#include <string>
#include <vector>

namespace micral {

struct Tty {
  std::deque<uint8_t> in;   // characters still to type
  std::string out;          // everything printed
  bool ready = false;       // a received character is waiting (status bit 0)
  uint8_t data = 0;
  uint64_t idle_polls = 0;  // status reads since the last character moved
  // Deliver the next typed character when the program next looks.
  void refill() {
    if (!ready && !in.empty()) {
      data = in.front();
      in.pop_front();
      ready = true;
    }
  }
};

// 256-byte LIFO with a pointer (io_map.md, stack card).
struct StackCard {
  uint8_t mem[256] = {};
  uint8_t ptr = 0;
  void push(uint8_t v) { mem[ptr++] = v; }
  uint8_t pop() { return mem[--ptr]; }
};

// Formatter-coupler + channel card (io_map.md, floppy disk). One byte moves
// disk -> channel every `byte_cycles` CPU cycles while a transfer runs.
struct Disk {
  std::vector<uint8_t> image;     // raw: track-major, 32 sectors of 128 bytes
  int tracks = 64, sectors = 32;
  int track = 0, sector = 0, index = 0;
  bool step_line = false, dir_up = false;
  bool transferring = false;
  StackCard channel;
  int counter = 0;
  uint64_t byte_cycles = 64, next_byte = 0;

  uint8_t data(int t, int s, int i) const {
    size_t at = (size_t(t) * sectors + s) * 128 + i;
    return at < image.size() ? image[at] : 0;
  }
  uint8_t checksum(int t, int s) const {
    uint8_t c = 0;
    for (int i = 0; i < 128; ++i) {
      uint8_t b = data(t, s, i);
      uint8_t v = ((b ^ c) ^ 0xff) & ((uint8_t)(b + c) ^ b);
      c = c + b + ((v & 0x80) ? 1 : 0);
    }
    return c;
  }
  uint8_t next_sector_byte() {
    int i = index++;
    if (i >= 2 && i < 130) return data(track, sector, i - 2);
    if (i == 130) return checksum(track, sector);
    return 0xff - i;
  }
  void command(uint8_t v) {
    int s = v & 0x1f;
    if (s != sector) { sector = s; index = 0; }
    dir_up = v & 0x20;
    bool st = v & 0x40;
    if (step_line && !st) {   // STEP falls: move one track
      if (dir_up) track = (track + 1) % tracks;
      else track = (track == 0 ? tracks : track) - 1;
      index = 0;
    }
    step_line = st;
  }
  uint8_t status() const {
    return (track == 0 ? 0x80 : 0) | (transferring ? 0x03 : 0);
  }
  void tick(uint64_t cycle) {
    if (!transferring || cycle < next_byte) return;
    next_byte = cycle + byte_cycles;
    if (counter > 0) {
      channel.push(next_sector_byte());
      --counter;
    }
    if (counter == 0) transferring = false;
  }
};

struct Board {
  uint8_t mem[0x4000] = {};
  uint8_t boot_rom[256] = {};
  bool boot_mapped = false;           // page-0 reads come from boot_rom
  bool rom_hi = false;                // 0x3800-0x3FFF read-only (monitor)
  Tty tty;
  StackCard stack;
  Disk disk;
  uint64_t cycle = 0;

  uint8_t read(uint16_t a, bool opcode_fetch) {
    a &= 0x3fff;
    uint8_t v = (boot_mapped && a < 0x100) ? boot_rom[a] : mem[a];
    if (opcode_fetch && v == 0x05) boot_mapped = false;   // RST 0 unmaps it
    return v;
  }
  void write(uint16_t a, uint8_t v) {
    a &= 0x3fff;
    if (rom_hi && a >= 0x3800) return;
    mem[a] = v;
  }
  uint8_t input(unsigned port, uint8_t a) {
    switch (port) {
    case 0:
      if (a == 0) { tty.ready = false; tty.idle_polls = 0; return tty.data; }
      if (a == 1) {
        tty.refill();
        ++tty.idle_polls;
        return (tty.ready ? 0x01 : 0) | 0x80;
      }
      if (a == 0xfe) return disk.status();
      if (a == 0xff) return disk.channel.ptr;
      return 0;
    case 5: return disk.channel.pop();
    case 6: return stack.pop();
    default: return 0;
    }
  }
  void output(unsigned port, uint8_t a) {
    switch (port) {
    case 8: tty.out.push_back(char(a)); tty.idle_polls = 0; break;
    case 20: disk.channel.ptr = a; break;
    case 21: disk.counter = a + 1; break;
    case 22: disk.transferring = true; disk.next_byte = cycle + disk.byte_cycles; break;
    case 23: disk.command(a); break;
    case 30: stack.push(a); break;
    default: break;
    }
  }
};

} // namespace micral
