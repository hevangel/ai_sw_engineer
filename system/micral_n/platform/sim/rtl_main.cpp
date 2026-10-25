// Micral N on the ai_hw_engineer 8008 RTL (Verilator). The CPU is
// design/intel_8008/src/intel_8008.sv; memory and the cards are board.h.
//
//   micral_rtl [options]
//     --boot FILE        256-byte boot ROM at 0x0000 (read until RST 0)
//     --rom FILE@ADDR    read-only image (e.g. the monitor at 0x3800)
//     --ram FILE@ADDR    preload RAM
//     --disk FILE        raw disk image (track-major, 32 x 128-byte sectors)
//     --start ADDR       feed `JMP ADDR` at reset, as the console does
//     --tty-in FILE      characters to type (sent as the program asks)
//     --tty-out FILE     write everything printed (default: stdout)
//     --max-cycles N     give up after N clock cycles (default 2e9)
//     --idle N           stop once input is used up and the program has
//                        polled the teletype N times without printing
//     --dump FILE@ADDR:LEN  write memory after the run
//     --regs             print PC and A at the end
//     --trace N          print PC, A and flags for the first N instructions
//   Exit status: 0 = halted or idle, 3 = max cycles, 2 = usage error.
#include "Vintel_8008.h"
#include "board.h"
#include "verilated.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <string>

using micral::Board;

static std::vector<uint8_t> slurp(const std::string &path) {
  std::ifstream f(path, std::ios::binary);
  if (!f) {
    std::fprintf(stderr, "micral_rtl: cannot read %s\n", path.c_str());
    std::exit(2);
  }
  return {std::istreambuf_iterator<char>(f), {}};
}

static void split_at(const std::string &arg, std::string &file, unsigned &addr) {
  auto at = arg.rfind('@');
  if (at == std::string::npos) {
    std::fprintf(stderr, "micral_rtl: expected FILE@ADDR, got %s\n", arg.c_str());
    std::exit(2);
  }
  file = arg.substr(0, at);
  addr = std::stoul(arg.substr(at + 1), nullptr, 0);
}

int main(int argc, char **argv) {
  Verilated::commandArgs(argc, argv);
  static Board b;
  long start = -1;
  uint64_t max_cycles = 2000000000ull, idle = 0;
  std::string tty_out, dump;
  bool regs = false;
  uint64_t trace = 0;

  for (int i = 1; i < argc; ++i) {
    std::string a = argv[i];
    auto next = [&]() -> std::string {
      if (i + 1 >= argc) { std::fprintf(stderr, "micral_rtl: %s needs a value\n", a.c_str()); std::exit(2); }
      return argv[++i];
    };
    if (a == "--boot") {
      auto d = slurp(next());
      std::memcpy(b.boot_rom, d.data(), std::min<size_t>(d.size(), 256));
      b.boot_mapped = true;
    } else if (a == "--rom" || a == "--ram") {
      std::string f; unsigned at;
      split_at(next(), f, at);
      auto d = slurp(f);
      for (size_t k = 0; k < d.size() && at + k < 0x4000; ++k) b.mem[at + k] = d[k];
      if (a == "--rom") b.rom_hi = true;
    } else if (a == "--disk") {
      b.disk.image = slurp(next());
    } else if (a == "--start") {
      start = std::stol(next(), nullptr, 0);
    } else if (a == "--tty-in") {
      for (uint8_t c : slurp(next())) b.tty.in.push_back(c == '\n' ? '\r' : c);
    } else if (a == "--tty-out") {
      tty_out = next();
    } else if (a == "--max-cycles") {
      max_cycles = std::stoull(next(), nullptr, 0);
    } else if (a == "--idle") {
      idle = std::stoull(next(), nullptr, 0);
    } else if (a == "--dump") {
      dump = next();
    } else if (a == "--trace") {
      trace = std::stoull(next(), nullptr, 0);
    } else if (a == "--regs") {
      regs = true;
    } else if (a[0] == '+') {
      // Verilator plusargs
    } else {
      std::fprintf(stderr, "micral_rtl: unknown option %s\n", a.c_str());
      return 2;
    }
  }

  auto *cpu = new Vintel_8008;
  cpu->clk = 0;
  cpu->rst_n = 0;
  cpu->ready = 1;
  cpu->interrupt_i = 0;
  cpu->interrupt_opcode = 0;
  cpu->mem_data_i = 0;
  cpu->io_data_i = 0;
  for (int k = 0; k < 4; ++k) { cpu->clk = !cpu->clk; cpu->eval(); }
  cpu->rst_n = 1;

  // Console substitution: the first three fetches read 0x44 lo hi.
  uint8_t subst[3] = {0x44, uint8_t(start & 0xff), uint8_t((start >> 8) & 0x3f)};
  int subst_left = start >= 0 ? 3 : 0;
  bool first_fetch = true;
  int rc = 3;

  for (b.cycle = 0; b.cycle < max_cycles; ++b.cycle) {
    cpu->clk = 0;
    cpu->eval();
    if (trace && cpu->retire) {
      --trace;
      std::fprintf(stderr, "pc=%04x a=%02x f=%x\n", cpu->pc_debug, cpu->a_debug,
                   cpu->flags_debug);
    }
    // Present this cycle's read data before the rising edge samples it.
    bool fetch = cpu->mem_read && (cpu->retire || first_fetch);
    if (cpu->mem_read) {
      if (subst_left) {
        cpu->mem_data_i = subst[3 - subst_left--];
      } else {
        cpu->mem_data_i = b.read(cpu->mem_addr, fetch);
      }
      if (fetch) first_fetch = false;
    }
    if (cpu->io_read) cpu->io_data_i = b.input(cpu->io_port, cpu->io_data_o);
    bool wr = cpu->mem_write, out = cpu->io_write;
    uint16_t wa = cpu->mem_addr;
    uint8_t wd = cpu->mem_data_o, port = cpu->io_port, ad = cpu->io_data_o;
    cpu->clk = 1;
    cpu->eval();
    if (wr) b.write(wa, wd);
    if (out) b.output(port, ad);
    b.disk.tick(b.cycle);
    if (cpu->halted) { rc = 0; break; }
    if (idle && b.tty.in.empty() && !b.tty.ready && b.tty.idle_polls >= idle) { rc = 0; break; }
  }

  if (tty_out.empty()) {
    std::fwrite(b.tty.out.data(), 1, b.tty.out.size(), stdout);
  } else {
    std::ofstream(tty_out, std::ios::binary) << b.tty.out;
  }
  if (!dump.empty()) {
    auto colon = dump.rfind(':');
    std::string f; unsigned at;
    split_at(dump.substr(0, colon), f, at);
    unsigned len = std::stoul(dump.substr(colon + 1), nullptr, 0);
    std::ofstream o(f, std::ios::binary);
    o.write(reinterpret_cast<char *>(b.mem + at), std::min(len, 0x4000u - at));
  }
  if (regs)
    std::fprintf(stderr, "micral_rtl: pc=%04x a=%02x cycles=%llu %s\n", cpu->pc_debug,
                 cpu->a_debug, (unsigned long long)b.cycle,
                 rc == 0 ? (cpu->halted ? "halted" : "idle") : "max-cycles");
  delete cpu;
  return rc;
}
