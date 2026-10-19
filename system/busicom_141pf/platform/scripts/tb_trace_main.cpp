// busicom_141pf primary-oracle (RTL) trace harness.
//
// Verilates the ai_hw_engineer whole-board RTL (busicom_141pf + intel_400{1,2,3,4})
// and runs ANY 4004 ROM image on it, emitting the port-level tape in EXACTLY the
// oracle/rom_port_trace.txt format. It is the RTL counterpart of chips_trace.rs:
// same schedule, same drum/TEST/index pacing (spin half-spin length, index pulse
// at spin 26, TEST high on even half-spins), same drum decode tables, so the two
// simulators are fed byte-identical stimulus and their output is directly
// comparable (any disagreement is a FINDING, not noise).
//
// The board's front-panel interface already does the keyboard-matrix decode, the
// 20-bit 4003 printer shift chain, RAM0/RAM1 edge capture and lamp latch in RTL,
// exposing one-shot hammer/advance events plus the captured 20-bit hammer word
// and drum position (hammer_data = {drum_pos[3:0], word[19:0]}). This harness
// drives keys/switches/TEST/drum exactly as tb/tb_top.sv does and polls the
// one-shot events once per drum half-spin, the SAME cadence as the reference DPI
// panel bridge (tb_top SYSTEM_DPI tick loop). Between polls it acks delivered
// events with a panel_tick toggle so the board re-arms, identical to the bridge.
//
// One machine cycle = 8 full clock periods (tb_top: 8 clocks of 10ns = 80ns).
//
// Env:
//   BUSICOM_SPIN  machine cycles per drum half-spin (default 1481, the reference
//                 value the oracle was captured at). A smaller value speeds up
//                 RTL runs; firmware behaviour is unchanged because all drum
//                 timings scale together (tb_top +spin). Must stay long enough
//                 for the firmware's per-sector print burst.
//
// Usage: tb_trace <schedule.txt>  (image pre-split into ./src/rom/rom_4001_N.hex)
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <fstream>
#include <sstream>
// Board selection (default: stock five-ROM/two-RAM board). The xl build writes
// a tiny generated header (board_sel.h) into the Verilator build dir and passes
//   -DBOARD_SEL_HEADER
// so this harness includes it; that header #defines BOARD_TOP / BOARD_HDR /
// BOARD_ROOT_HDR / LAMP_MEMBER. Using a generated header instead of -D string
// literals avoids the quote-mangling that happens when a "..."-valued -D is
// threaded through Verilator -> make -> shell. With no defines at all this file
// is byte-identical to the stock build. BOARD_TOP names the Verilated top;
// V##BOARD_TOP is its class; BOARD_HDR/BOARD_ROOT_HDR are its generated headers;
// LAMP_MEMBER is the --public-flat-rw root member holding the full 4-bit RAM1
// lamp port (incl. spec-A9 bit 3).
#ifdef BOARD_SEL_HEADER
#include "board_sel.h"
#endif
#ifndef BOARD_TOP
#define BOARD_TOP busicom_141pf
#endif
#ifndef BOARD_HDR
#define BOARD_HDR "Vbusicom_141pf.h"
#endif
#ifndef BOARD_ROOT_HDR
#define BOARD_ROOT_HDR "Vbusicom_141pf___024root.h"
#endif
#ifndef LAMP_MEMBER
#define LAMP_MEMBER busicom_141pf__DOT__ram1_port
#endif
#define VCAT2(a,b) a##b
#define VCAT(a,b) VCAT2(a,b)
#define VTOP VCAT(V, BOARD_TOP)
#include BOARD_HDR
#include BOARD_ROOT_HDR
#include "verilated.h"

static const char *DIGITS[13] = {"0","1","2","3","4","5","6","7","8","9",".",".","-"};
static const char *SA[13] = {"\u25c7","+","-","\u00d7","\u00f7","M+","M-","^","=","\u221a","%","C","R"};
static const char *SB[13] = {"#","*","\u2160","\u2161","\u2162","M+","M-","T","K","E","Ex","C","M"};

struct Row {
    std::string cells[18]; // 0..14 numeric, 16 symA, 17 symB (15 unused, mirrors oracle)
    Row() { reset(); }
    void reset() { for (int i = 0; i < 18; i++) cells[i] = " "; }
    bool any() const { for (int i = 0; i < 18; i++) if (cells[i] != " ") return true; return false; }
};

struct Sim {
    VTOP *dut;
    int spin_cnt = 0;      // half-spin counter (tb_top spin_cnt)
    int drum_pos = 0;      // 0..12 position reported to the board
    Row row;
    bool red = false;
    uint8_t panel_tick = 0;
    int spin_cycles = 1481;

    Sim() {
        const char *e = getenv("BUSICOM_SPIN");
        if (e && *e) spin_cycles = atoi(e);
        dut = new VTOP;
        dut->clk = 0; dut->rst_n = 0; dut->clr_n = 0;
        dut->test_i = 0; dut->drum_idx_i = 0; dut->panel_tick_i = 0;
        dut->keys_mask_i = 0; dut->precision_i = 0; dut->rounding_i = 0;
        dut->paper_btn_i = 0; dut->drum_pos_i = 0;
        dut->eval();
    }
    ~Sim() { dut->final(); delete dut; }

    // one 4004 machine cycle = 8 full clock periods = 16 clk edges.
    void machine_cycle() {
        for (int i = 0; i < 16; i++) { dut->clk = !dut->clk; dut->eval(); }
    }

    void clocks(int n) { for (int i = 0; i < n; i++) machine_cycle(); }

    void power_on() {
        dut->rst_n = 0; dut->clr_n = 0;
        clocks(2);
        dut->rst_n = 1; dut->eval();
        clocks(3);
        dut->clr_n = 1; dut->eval();
    }

    void reset_example() {
        dut->rst_n = 0; dut->clr_n = 0;
        dut->keys_mask_i = 0; dut->precision_i = 0; dut->rounding_i = 0;
        dut->test_i = 0; dut->drum_idx_i = 0; dut->panel_tick_i = 0;
        dut->drum_pos_i = 0; dut->paper_btn_i = 0;
        spin_cnt = 0; drum_pos = 0; red = false; row.reset(); panel_tick = 0;
        dut->eval();
        power_on();
    }

    void decode_hammer(uint32_t data24) {
        int dpos = (data24 >> 20) & 0xF;
        uint32_t word = data24 & 0xFFFFF;
        if (dpos > 12) dpos = 12;
        for (int i = 0; i < 15; i++)
            if (word & (1u << (i + 3))) row.cells[i] = DIGITS[dpos];
        if (word & 1) row.cells[16] = SA[dpos];
        if (word & 2) row.cells[17] = SB[dpos];
    }

    void flush_row() {
        if (row.any()) {
            std::string num;
            for (int i = 0; i < 15; i++) num += row.cells[i];
            size_t a = num.find_first_not_of(' ');
            size_t b = num.find_last_not_of(' ');
            num = (a == std::string::npos) ? "" : num.substr(a, b - a + 1);
            printf("ROW|%s|%s|%s|%s\n", num.c_str(), row.cells[16].c_str(),
                   row.cells[17].c_str(), red ? "true" : "false");
        }
        row.reset();
        red = false;
    }

    // Poll + ack once per half-spin, the reference DPI bridge cadence. The
    // board delivers at most one latched hammer and one advance per tick
    // window (one printer sector per half-spin); red/lamps are levels.
    void poll_tick() {
        if (dut->red_o) red = true;
        // advance flushes the accumulated row BEFORE a new hammer of the next
        // line folds in (chips: RAM0 bit3 rising-edge flush, then bit1 capture).
        if (dut->advance_evt_o) flush_row();
        if (dut->hammer_evt_o) decode_hammer(dut->hammer_data_o);
        // ack: toggle panel_tick and run a cycle so tick_pulse clears latches
        panel_tick ^= 1;
        dut->panel_tick_i = panel_tick;
        machine_cycle();
        if (dut->red_o) red = true;
    }

    // Advance drum/TEST exactly as chips_trace.rs does at the end of each
    // schedule tick: set TEST = (spin even), drive the index pulse + reset at
    // spin 26, and report the drum character chips decodes as (spin/2+12)%13.
    // Driving the board's drum_pos_i with that same expression keeps the RTL's
    // captured hammer position byte-identical to the secondary oracle's decode.
    void half_spin() {
        dut->test_i = (spin_cnt % 2 == 0) ? 1 : 0;
        if (spin_cnt % 2 == 0) {
            if (spin_cnt == 26) { dut->drum_idx_i = 1; spin_cnt = 0; }
            else                { dut->drum_idx_i = 0; }
        }
        spin_cnt += 1;
        drum_pos = (spin_cnt / 2 + 12) % 13;
        dut->drum_pos_i = drum_pos;
        dut->eval();
    }

    uint8_t lamps() {
        // The oracle's LAMPS value is the FULL 4-bit RAM1 output port
        // (chips: b.rams[1].ports.value()), including the undocumented bit 3
        // the ROM sets on overflow (nibble 10) and negative totals (nibble 12)
        // -- spec A9. The board's lamps_o port is only 3 bits wide (memory /
        // overflow / negative), so bit 3 is NOT observable there; we read the
        // internal 4-bit ram1_port (Verilator --public-flat-rw) to see it.
        return dut->rootp->LAMP_MEMBER & 0xF;
    }
};

int main(int argc, char **argv) {
    Verilated::commandArgs(argc, argv);
    if (argc < 2) { fprintf(stderr, "usage: tb_trace <schedule.txt>\n"); return 2; }
    std::ifstream sched(argv[1]);
    if (!sched) { fprintf(stderr, "cannot open schedule %s\n", argv[1]); return 2; }

    Sim sim;
    sim.power_on();

    std::string line;
    while (std::getline(sched, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (line.rfind("#EXAMPLE", 0) == 0) {
            printf("%s\n", line.c_str());
            sim.reset_example();
            continue;
        }
        if (!line.empty() && line[0] == '#') {
            printf("%s\n", line.c_str());
            if (line.rfind("#STEP", 0) == 0)
                printf("LAMPS|%d\n", sim.lamps());
            continue;
        }
        if (line.empty()) continue;
        int precision, rounding, key, duration;
        { std::istringstream is(line);
          if (!(is >> precision >> rounding >> key >> duration)) continue; }
        for (int tick = 0; tick < duration; tick++) {
            int current = (tick < 64) ? key : 0;
            uint32_t mask = 0;
            if (current >= 129 && current <= 160) mask = 1u << (current - 129);
            sim.dut->keys_mask_i = mask;
            sim.dut->precision_i = precision & 0xF;
            sim.dut->rounding_i = rounding & 0xF;
            sim.dut->eval();
            sim.clocks(sim.spin_cycles);
            sim.poll_tick();
            sim.half_spin();
        }
    }
    return 0;
}
