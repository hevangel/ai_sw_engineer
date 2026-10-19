# busicom_141pf xl-board memory test

Proves the memory the **xl** board adds over the stock board is addressable, on
both whole-machine simulators. The xl board (`platform/rtl/busicom_141pf_xl.sv`,
`platform/link_xl.ld`, `memory_map.md` "Extended (xl) board") keeps every
Busicom I/O role identical and only adds ROM/RAM: 16 × 4001 (4096 B) and
16 × 4002 (640 B, 4 CM-RAM banks).

## Programs

| Source | Sweeps | Runs on |
|---|---|---|
| `memtest_xl.s` | CM-RAM bank 0, all 4 × 4002 (chips 0–3), 4 regs, 16 chars = **256 cells** | chips **and** RTL |
| `memtest_xl_allbanks.s` | **All 4 banks** × 4 chips × 4 regs × 16 chars = **1024 cells** (DCL-selected per bank) | **RTL only** |

Each cell is written a pattern derived from its address, then read back and
compared; any mismatch sets a FAIL flag. The verdict is driven to the **RAM1
lamp port** (sampled by the runner as `LAMPS|n`, see `../../io_map.md`):

- `LAMPS|7` (0b0111) = **PASS** (every swept cell verified)
- `LAMPS|1` (0b0001) = **FAIL** (≥1 mismatch)

then the program loops forever re-driving the verdict so every sampled step
shows it. These are standalone power-on probes: linked with `crt0.s` only (no
HAL, no Rust firmware, no codegen backend), packed into a 4096-byte 16-page
image by `llvm-i4004/asm/i4004_pack.py --max 4096`.

## Build

llvm-mc lives in the ai-sw-engineer image (on the `ai-sw-llvm-build` volume):

```bash
docker run --rm -v ai-sw-llvm-build:/llvm-build -v "$PWD:/ws" -w /ws \
  ai-sw-engineer:latest \
  bash system/busicom_141pf/platform/runtime/tests/build_memtest_xl.sh \
    build/memtest_xl.bin               # bank-0 test (chips + RTL)
# full 4-bank (RTL-only) image:
#   ...build_memtest_xl.sh build/memtest_xl_allbanks.bin memtest_xl_allbanks.s
```

Images land in `tests/build/` (gitignored).

## Run

```bash
# minimal no-key schedule: settle, then sample LAMPS at a step
printf '#EXAMPLE memtest\n0 0 0 20\n#RESET\n0 0 0 6\n#STEP 0\n' > sched.txt

# chips (secondary), bank-0 image:
run_image.sh secondary build/memtest_xl.bin out.txt sched.txt --board xl   # -> LAMPS|7

# RTL (primary), bank-0 image (ai-hw-engineer image, verilator):
BUSICOM_SPIN=900 run_image.sh primary build/memtest_xl.bin out.txt sched.txt --board xl  # -> LAMPS|7

# RTL (primary), full 4-bank image:
BUSICOM_SPIN=1200 run_image.sh primary build/memtest_xl_allbanks.bin out.txt sched.txt --board xl  # -> LAMPS|7
```

## Results (verified)

| Test | Simulator | ram_count | Verdict |
|---|---|---|---|
| bank-0 256 cells | chips (secondary) | 4 | **PASS** (`LAMPS|7`) |
| bank-0 256 cells | RTL (primary) | 16 | **PASS** (`LAMPS|7`) |
| full 4-bank 1024 cells | RTL (primary) | 16 | **PASS** (`LAMPS|7`) |

**Negative control:** running `memtest_xl.bin` through the chips adapter with
`ram_count=2` (so the two ADDED bank-0 chips 2,3 are absent) yields `LAMPS|1`
(**FAIL**) — the test genuinely distinguishes present from absent memory, it is
not a constant PASS.

## FINDING — chips model caps at one CM-RAM bank

The unmodified external `chips` board model stores a 4002's identity in a 2-bit
`page_mask` and `Board::new` sets it from the raw chip index, so it **panics for
a chip index ≥ 4** — it can represent only a single CM-RAM bank of up to four
4002s. `run_image.sh --board xl secondary` therefore runs with `ram_count=4`
(bank 0 fully populated). The full 16-chip / 4-bank xl RAM, and the DCL bank
decode, are validated on the **RTL** simulator instead (`memtest_xl_allbanks`,
PASS above). We do not edit `external/` (read/run only), so this is a documented
capability gap of the secondary oracle, not a defect to patch.
