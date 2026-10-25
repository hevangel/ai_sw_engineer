# Emulator verification

1. Reject empty/oversized images and malformed schedules with useful errors.
2. Execute small hand-encoded 4004 programs through real port instructions to
   check ROM input, RAM lamp output and reset isolation, rather than calling
   firmware's calculator logic.
3. With the user-supplied original ROM, replay all 42 committed manual examples
   and compare every trace payload line against `rom_port_trace.txt`. This
   opt-in test requires `BUSICOM_ROM`; normal tests never need copyrighted ROMs.
4. Run host format and clippy checks. Build the CLI and exercise schedule input
   and error exit status. Firmware and LLVM sources are unchanged.

The exact original-ROM regression is the behavior gate. This core also drives
the old secondary adapter; independent evidence is the RTL validation recorded
in the oracle provenance, not a claim that two wrappers are independent.
