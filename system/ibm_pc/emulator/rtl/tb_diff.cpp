// Differential runner: executes vector files (format 8086V4) on the unmodified
// RTL in $AI_HW_ENGINEER/design/intel_8086 and reports EVERY mismatch, where the
// hardware project's own runner stops at the first.
//
// It reuses the RTL testbench (Bench, record reader, comparisons) by including
// tb_vectors.cpp with its main() renamed; nothing in the sibling repository is
// modified. Build and run with scripts/rtl_diff.sh.
#define main rtl_original_main
#include "tb_vectors.cpp"
#undef main

#include <map>

static void diff(Bench &b, const char *path, unsigned show, unsigned &total_pass, unsigned &total_fail) {
    std::ifstream input(path, std::ios::binary);
    if (!input) throw std::runtime_error(std::string("missing vector file ") + path);
    std::string magic(8, '\0');
    input.read(magic.data(), 8);
    if (magic != std::string("8086V4\0\0", 8)) throw std::runtime_error("bad vector header");
    unsigned pass = 0, fail = 0;
    std::map<std::string, unsigned> reasons;
    while (input.peek() != EOF) {
        std::string label;
        auto n = number(input, 2);
        label.resize(n);
        input.read(label.data(), n);
        auto initial = registers(input), expected = registers(input);
        auto mask = number(input, 2);
        auto initial_ram = ram(input), expected_ram = ram(input);
        auto expected_writes = pairs(input);
        auto byte_masks = ram(input);
        auto initial_queue = pairs(input);
        (void)initial_queue;
        try {
            for (auto [a, v] : initial_ram)
                if (!expected_ram.count(a)) expected_ram[a] = v;
            b.reset();
            b.load(initial);
            for (auto [a, v] : initial_ram) b.set(a, v);
            b.retire();
            compare(b.read(), expected, mask);
            for (auto [a, v] : expected_ram)
                if (((b.memory[a] ^ v) & (byte_masks.count(a) ? byte_masks[a] : 255)) != 0)
                    throw std::runtime_error("memory mismatch at " + std::to_string(a));
            for (auto [a, v] : b.writes)
                if (!expected_ram.count(a) ||
                    ((expected_ram[a] ^ v) & (byte_masks.count(a) ? byte_masks[a] : 255)) != 0)
                    throw std::runtime_error("unlisted/incorrect write at " + std::to_string(a));
            bool write_match = b.ordered_writes.size() == expected_writes.size();
            for (size_t i = 0; write_match && i < expected_writes.size(); ++i) {
                auto [a, v] = expected_writes[i];
                auto [actual_a, actual_v] = b.ordered_writes[i];
                write_match = a == actual_a && ((v ^ actual_v) & (byte_masks.count(a) ? byte_masks[a] : 255)) == 0;
            }
            if (!write_match) throw std::runtime_error("ordered byte writes differ");
            if (!b.dut.halted_o) next_nop(b, expected);
            else {
                b.tick();
                if (b.request() || b.dut.retire_o) throw std::runtime_error("HLT did not stay idle");
            }
            ++pass;
        } catch (const std::exception &e) {
            ++fail;
            std::string reason = e.what();
            auto key = label.substr(0, label.find(' ', 5)) + " " + reason.substr(0, reason.find(' '));
            if (reasons[key]++ < show) std::cout << "FAIL " << label << ": " << reason << "\n";
        }
    }
    std::cout << "RESULT " << path << " pass=" << pass << " fail=" << fail << "\n";
    total_pass += pass;
    total_fail += fail;
}

int main(int argc, char **argv) {
    Verilated::commandArgs(argc, argv);
    unsigned show = std::getenv("DIFF_SHOW") ? std::atoi(std::getenv("DIFF_SHOW")) : 2;
    unsigned pass = 0, fail = 0;
    try {
        Bench b;
        for (int i = 1; i < argc; ++i) diff(b, argv[i], show, pass, fail);
        b.dut.final();
    } catch (const std::exception &error) {
        std::cerr << "ERROR: " << error.what() << '\n';
        return 2;
    }
    std::cout << "TOTAL pass=" << pass << " fail=" << fail << "\n";
    return fail ? 1 : 0;
}
