# llvm-i4004 final report

| Check | Result |
|---|---|
| MC encode tests | TODO n/N |
| Codegen tests | TODO n/N |
| Smoke programs on simulator | TODO n/N |
| Titles built with this backend | TODO |

## Known gaps

Milestone 1 (specs) complete; no backend code yet (TableGen/C++/runtime are
milestones 2–4). Open design questions tracked in `spec/abi.md` §7 — notably
the `i4`-as-non-addressable-value-width legalizer question (#3), the soft-stack
RAM base (#4, blocked on `platform/link.ld`), and the provisional ELF
`e_machine=0x4004` (#5). 16/32-bit runtime helpers are unverified until a
non-firmware title needs them.
