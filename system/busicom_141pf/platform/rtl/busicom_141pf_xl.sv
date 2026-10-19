`timescale 1ns/1ps
// ============================================================================
// BUSICOM 141-PF — EXTENDED ("xl") board top.
//
// Derived from the authoritative stock board top
//   ../ai_hw_engineer/system/busicom_141pf/src/busicom_141pf.sv
// (same user's repo; deriving a variant top from its RTL into this tree is
// permitted — the stock file is the cited source). This variant INSTANTIATES
// THE SAME intel_4004 / intel_4001 / intel_4002 / intel_4003 modules by path
// and keeps every Busicom I/O role byte-identical; the ONLY changes are more
// memory:
//
//   ROM : 5 x 4001  ->  16 x 4001  (chips #0-#15, 0x000-0xFFF, 4096 bytes).
//         Chips #0-#2 keep their Busicom port roles (shifter clocks /
//         keyboard columns / drum index+paper); chips #3-#15 have no I/O
//         wired (port strapped 0), exactly as #3/#4 on the stock board.
//   RAM : 2 x 4002   ->  16 x 4002 (4 banks x 4 chips, CM-RAM0..3).
//         Bank 0 chip 0 = RAM0 role (printer red/hammers/paper port),
//         bank 0 chip 1 = RAM1 role (lamps port) — UNCHANGED from stock.
//         Bank 0 chips 2-3 and banks 1-3 are added data/stack/heap memory
//         with their output ports unused.
//
// The ROM-port select alias translator (SRC X2 nibble mod 5) and the entire
// front-panel section (keyboard decode, 4003 printer chain, RAM0/RAM1 edge
// capture, lamps) are copied verbatim from the stock top, so the original ROM
// reproduces rom_port_trace.txt line-for-line on this variant — extra chips
// are invisible to a program that never fetches past 0x4FF or selects a RAM
// chip/bank it was never wired to use.
//
// The 4003 printer/keyboard shifters are still clocked from ROM0's port, and
// the lamp/printer edge capture still watches ONLY bank-0 chips 0/1, so the
// added RAM chips cannot perturb the tape.
// ============================================================================
module busicom_141pf_xl (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        clr_n,        // historical CL: clears 4001 I/O latches
    input  logic        test_i,       // printer drum timing -> CPU TEST pin
    input  logic        drum_idx_i,   // index sensor -> ROM2 bit 0
    input  logic        panel_tick_i, // host bridge tick (level toggles)
    // front panel -> board (quasi-static, host provided)
    input  logic [31:0] keys_mask_i,  // bit i = scancode 129+i pressed
    input  logic [3:0]  precision_i,  // decimal-point selector
    input  logic [3:0]  rounding_i,   // 0 float, 1 round, 8 truncate
    input  logic        paper_btn_i,  // manual paper advance button
    // board -> front panel
    output logic        hammer_evt_o, // one-shot: hammer_data_o is fresh
    output logic [23:0] hammer_data_o,// {drum_pos[3:0], shifter word[19:0]}
    output logic        advance_evt_o,// one-shot: paper advanced one line
    output logic        red_o,        // red/black ribbon level (RAM0 bit0)
    output logic [2:0]  lamps_o,      // {negative, overflow, memory}
    input  logic [3:0]  drum_pos_i,   // current drum character position 0..12
    output logic        key_seen_o,   // one-shot: firmware sampled the key
    output logic [9:0]  kb_scan_o     // keyboard scan one-low (4003 #0)
);

    localparam int NROM = 16;  // xl: 16 x 4001
    localparam int NRAM = 16;  // xl: 16 x 4002 (4 banks x 4 chips)

    // ------------------------------------------------------------------------
    // Shared 4-bit data bus. Every chip drives 4'h0 while its oe is low, so an
    // OR tree resolves the bus (exactly one driver enabled at a time).
    // ------------------------------------------------------------------------
    logic [3:0] data_bus;

    logic [3:0] cpu_data_o;
    logic       cpu_data_oe;
    logic [3:0] rom_data_o   [NROM];
    logic       rom_data_oe  [NROM];
    logic [3:0] ram_data_o   [NRAM];
    logic       ram_data_oe  [NRAM];

    always_comb begin
        data_bus = cpu_data_oe ? cpu_data_o : 4'h0;
        for (int i = 0; i < NROM; i++)
            if (rom_data_oe[i]) data_bus = data_bus | rom_data_o[i];
        for (int i = 0; i < NRAM; i++)
            if (ram_data_oe[i]) data_bus = data_bus | ram_data_o[i];
    end

    // ------------------------------------------------------------------------
    // ROM-port select alias translator (copied verbatim from the stock top).
    //
    // The firmware addresses the five 4001 I/O ports by (SRC X2 nibble mod 5):
    // it selects nibbles 0/1/2/4/5/7, where 5 aliases to port 0 and 7 to port
    // 2. The stock 4001 compares the whole nibble against its strapped chip
    // number, so the board rewrites ROM-port SRC operands before the chips
    // snoop them. Only SRC-X2 words under CM-ROM are translated. On the xl
    // board the translation is UNCHANGED (still mod 5) so the original ROM's
    // port usage is identical; the added ROMs #5-#15 therefore never see a
    // translated I/O select and keep their ports idle.
    // ------------------------------------------------------------------------
    localparam logic [2:0] PhA1 = 3'd0;
    localparam logic [2:0] PhM1 = 3'd3;
    localparam logic [2:0] PhM2 = 3'd4;
    localparam logic [2:0] PhX2 = 3'd6;
    localparam logic [2:0] PhX3 = 3'd7;

    logic [2:0] xlate_phase;
    logic [3:0] xlate_opr, xlate_opa;

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            xlate_phase <= PhA1;
            xlate_opr   <= 4'h0;
            xlate_opa   <= 4'h0;
        end else begin
            if (sync) xlate_phase <= PhA1 + 3'd1;
            else if (xlate_phase == PhX3) xlate_phase <= PhA1;
            else xlate_phase <= xlate_phase + 3'd1;

            if (xlate_phase == PhM1) xlate_opr <= data_bus;
            if (xlate_phase == PhM2) xlate_opa <= data_bus;
        end
    end

    wire xlate_src_x2 = (xlate_phase == PhX2) &&
                        (xlate_opr == 4'h2) && xlate_opa[0];

    wire [3:0] xlate_raw = data_bus;
    wire       xlate_remap = xlate_src_x2 && (xlate_raw > 4'd4);

    logic [3:0] data_chips;
    always_comb begin
        case ({xlate_remap, xlate_raw})
            default: data_chips = xlate_raw;
            5'b1_0101: data_chips = 4'h0;  // 5 -> 0
            5'b1_0110: data_chips = 4'h1;  // 6 -> 1
            5'b1_0111: data_chips = 4'h2;  // 7 -> 2
            5'b1_1000: data_chips = 4'h3;  // 8 -> 3
            5'b1_1001: data_chips = 4'h4;  // 9 -> 4
            5'b1_1010: data_chips = 4'h0;  // 10 -> 0
            5'b1_1011: data_chips = 4'h1;  // 11 -> 1
            5'b1_1100: data_chips = 4'h2;  // 12 -> 2
            5'b1_1101: data_chips = 4'h3;  // 13 -> 3
            5'b1_1110: data_chips = 4'h4;  // 14 -> 4
            5'b1_1111: data_chips = 4'h0;  // 15 -> 0
        endcase
    end

    // ------------------------------------------------------------------------
    // CPU
    // ------------------------------------------------------------------------
    logic sync, cm_rom;
    logic [3:0] cm_ram;

    intel_4004 u_cpu (
        .clk     (clk),
        .rst_n   (rst_n),
        .test_i  (test_i),
        .data_i  (data_bus),
        .data_o  (cpu_data_o),
        .data_oe (cpu_data_oe),
        .sync    (sync),
        .cm_rom  (cm_rom),
        .cm_ram  (cm_ram)
    );

    // ------------------------------------------------------------------------
    // ROMs: chips #0-#15. #0-#2 keep their Busicom port roles; #3-#15 have no
    // I/O wired (port_i = 0). The mask files are rom_4001_0.hex .. _15.hex
    // (split_rom.py emits all present; missing high chips are zero masks).
    // ------------------------------------------------------------------------
    logic [3:0] rom0_port;
    logic [3:0] kb_col;

    // ROM0: drives the 4003 shifters (IO_DIR 0111).
    intel_4001 #(
        .CHIP_NO  (4'h0),
        .IO_DIR   (4'b0111),
        .ROM_FILE ("src/rom/rom_4001_0.hex")
    ) u_rom0 (
        .clk(clk), .rst_n(rst_n), .clr_n(clr_n),
        .data_i(data_chips), .data_o(rom_data_o[0]), .data_oe(rom_data_oe[0]),
        .sync(sync), .cm_rom(cm_rom),
        .port_i(4'h0), .port_o(rom0_port), .port_oe()
    );
    // ROM1: keyboard matrix columns (all inputs).
    intel_4001 #(
        .CHIP_NO  (4'h1),
        .IO_DIR   (4'b0000),
        .ROM_FILE ("src/rom/rom_4001_1.hex")
    ) u_rom1 (
        .clk(clk), .rst_n(rst_n), .clr_n(clr_n),
        .data_i(data_chips), .data_o(rom_data_o[1]), .data_oe(rom_data_oe[1]),
        .sync(sync), .cm_rom(cm_rom),
        .port_i(kb_col), .port_o(), .port_oe()
    );
    // ROM2: bit3 = paper button, bit0 = drum index pulse.
    intel_4001 #(
        .CHIP_NO  (4'h2),
        .IO_DIR   (4'b0000),
        .ROM_FILE ("src/rom/rom_4001_2.hex")
    ) u_rom2 (
        .clk(clk), .rst_n(rst_n), .clr_n(clr_n),
        .data_i(data_chips), .data_o(rom_data_o[2]), .data_oe(rom_data_oe[2]),
        .sync(sync), .cm_rom(cm_rom),
        .port_i({paper_btn_i, 2'b00, drum_idx_i}), .port_o(), .port_oe()
    );

    // ROMs #3-#15: no I/O wired. Generated so all 16 mask slots load and the
    // full 0x000-0xFFF program space is addressable.
    genvar r;
    generate
        for (r = 3; r < NROM; r++) begin : g_rom_hi
            localparam logic [3:0] CN = r[3:0];
            // $sformatf builds the per-chip mask file name at elaboration.
            intel_4001 #(
                .CHIP_NO  (CN),
                .IO_DIR   (4'b0000),
                .ROM_FILE ($sformatf("src/rom/rom_4001_%0d.hex", r))
            ) u_rom_hi (
                .clk(clk), .rst_n(rst_n), .clr_n(clr_n),
                .data_i(data_chips), .data_o(rom_data_o[r]),
                .data_oe(rom_data_oe[r]),
                .sync(sync), .cm_rom(cm_rom),
                .port_i(4'h0), .port_o(), .port_oe()
            );
        end
    endgenerate

    // ------------------------------------------------------------------------
    // RAM: 16 x 4002 across the four CM-RAM banks (DCL selected). Per bank the
    // quad is {4002-1 P0=0, 4002-1 P0=1, 4002-2 P0=0, 4002-2 P0=1} = chips
    // 0,1,2,3. Bank 0 chip 0 (RAM0) and chip 1 (RAM1) keep the printer/lamp
    // I/O roles; every other chip's output port is unused.
    // ------------------------------------------------------------------------
    logic [3:0] ram_port [NRAM];

    genvar b, c;
    generate
        for (b = 0; b < 4; b++) begin : g_bank
            for (c = 0; c < 4; c++) begin : g_chip
                localparam int IDX = b*4 + c;
                localparam logic V1  = (c < 2) ? 1'b1 : 1'b0;  // chips 0,1:4002-1 2,3:4002-2
                localparam logic PO  = (c & 1) ? 1'b1 : 1'b0;  // P0 strap
                intel_4002 #(.Variant1(V1)) u_ram (
                    .clk(clk), .rst_n(rst_n),
                    .data_i(data_bus),
                    .data_o(ram_data_o[IDX]), .data_oe(ram_data_oe[IDX]),
                    .sync(sync), .cm_ram_i(cm_ram[b]),
                    .po_i(PO),
                    .io_o(ram_port[IDX])
                );
            end
        end
    endgenerate

    // Printer/red port = bank 0 chip 0; lamps port = bank 0 chip 1. These are
    // the ONLY two RAM ports the front panel observes, identical to stock.
    wire [3:0] ram0_port = ram_port[0];
    wire [3:0] ram1_port = ram_port[1];

    // Full 4-bit RAM1 (lamp) output port, exposed as a declared net so the
    // Verilator harness can read it by a stable flattened name
    // (busicom_141pf_xl__DOT__ram1_port4) with --public-flat-rw. The public
    // panel port lamps_o is only 3 bits wide, so the undocumented bit 3
    // (spec A9, set on overflow/negative) is observable only here, matching
    // the stock harness's read of busicom_141pf__DOT__ram1_port.
    logic [3:0] ram1_port4;
    always_comb ram1_port4 = ram_port[1];

    // ------------------------------------------------------------------------
    // 4003 shift registers, clocked from the ROM0 port lines (unchanged).
    // ------------------------------------------------------------------------
    logic sh1_so;
    logic [9:0] sh1_q, sh2_q;

    intel_4003 #(.WIDTH(10)) u_sh_keyboard (
        .clk(clk), .rst_n(rst_n),
        .cp_i(rom0_port[0]), .data_in_i(rom0_port[1]), .en_i(1'b1),
        .q_o(kb_scan_o), .so_o()
    );
    intel_4003 #(.WIDTH(10)) u_sh_printer_lo (
        .clk(clk), .rst_n(rst_n),
        .cp_i(rom0_port[2]), .data_in_i(rom0_port[1]), .en_i(1'b1),
        .q_o(sh1_q), .so_o(sh1_so)
    );
    intel_4003 #(.WIDTH(10)) u_sh_printer_hi (
        .clk(clk), .rst_n(rst_n),
        .cp_i(rom0_port[2]), .data_in_i(sh1_so), .en_i(1'b1),
        .q_o(sh2_q), .so_o()
    );

    // ------------------------------------------------------------------------
    // Front panel: keyboard matrix decode (copied verbatim from stock top).
    // ------------------------------------------------------------------------
    always_comb begin
        case (~kb_scan_o)
            10'b0000000001:
                kb_col = {keys_mask_i[3], keys_mask_i[2], keys_mask_i[1],
                          keys_mask_i[0]};
            10'b0000000010:
                kb_col = {keys_mask_i[7], keys_mask_i[6], keys_mask_i[5],
                          keys_mask_i[4]};
            10'b0000000100:
                kb_col = {keys_mask_i[11], keys_mask_i[10], keys_mask_i[9],
                          keys_mask_i[8]};
            10'b0000001000:
                kb_col = {keys_mask_i[15], keys_mask_i[14], keys_mask_i[13],
                          keys_mask_i[12]};
            10'b0000010000:
                kb_col = {keys_mask_i[19], keys_mask_i[18], keys_mask_i[17],
                          keys_mask_i[16]};
            10'b0000100000:
                kb_col = {keys_mask_i[23], keys_mask_i[22], keys_mask_i[21],
                          keys_mask_i[20]};
            10'b0001000000:
                kb_col = {keys_mask_i[27], keys_mask_i[26], keys_mask_i[25],
                          keys_mask_i[24]};
            10'b0010000000:
                kb_col = {keys_mask_i[31], keys_mask_i[30], keys_mask_i[29],
                          keys_mask_i[28]};
            10'b0100000000: kb_col = precision_i;
            10'b1000000000: kb_col = rounding_i;
            default: kb_col = 4'h0;
        endcase
    end

    // ------------------------------------------------------------------------
    // Front panel: printer drum position and event capture (copied verbatim).
    // Watches ONLY bank-0 chips 0/1, so added RAM cannot perturb the tape.
    // ------------------------------------------------------------------------
    logic [1:0] ram0_prev;
    logic [1:0] tick_prev;
    logic key_seen;

    wire hammer_edge  = (ram0_prev[0] == 1'b0) && (ram0_port[1] == 1'b1);
    wire advance_edge = (ram0_prev[1] == 1'b0) && (ram0_port[3] == 1'b1);
    wire tick_pulse   = (panel_tick_i != tick_prev[0]);
    wire kbd_sampled  = u_rom1.s_rdr && u_rom1.io_selected &&
                        u_rom1.phase >= 6 && kb_col != 4'h0;

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            ram0_prev  <= 2'b00;
            tick_prev  <= 2'b00;
            key_seen   <= 1'b0;
            hammer_evt_o <= 1'b0;
            hammer_data_o <= 24'h0;
            advance_evt_o <= 1'b0;
            red_o       <= 1'b0;
            lamps_o     <= 3'b000;
        end else begin
            ram0_prev <= {ram0_port[3], ram0_port[1]};
            tick_prev <= {tick_prev[0], panel_tick_i};

            if (tick_pulse) begin
                hammer_evt_o  <= 1'b0;
                advance_evt_o <= 1'b0;
                key_seen      <= 1'b0;
            end
            if (hammer_edge) begin
                hammer_evt_o  <= 1'b1;
                hammer_data_o <= {drum_pos_i, sh2_q, sh1_q};
            end
            if (advance_edge)
                advance_evt_o <= 1'b1;
            if (kbd_sampled)
                key_seen <= 1'b1;

            red_o   <= ram0_port[0];
            lamps_o <= ram1_port[2:0];
        end
    end

    assign key_seen_o = key_seen;

endmodule
