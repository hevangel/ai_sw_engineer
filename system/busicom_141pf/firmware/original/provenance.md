# Busicom 141-PF firmware — original provenance

| File | Source | SHA-256 | Size | Redistribution |
|---|---|---|---|---|
| `rom_141pf_combined.bin` (5 × 256-byte 4001 masks, chip 0 first) | `$AI_HW_ENGINEER/system/busicom_141pf/spec/reference/rom_141pf_combined.bin`, extracted from the ROM array in [veniamin-ilmer/busicom](https://github.com/veniamin-ilmer/busicom) `src/lib.rs` (also `external/busicom`) | `c6af325d64b0895a8c6f7298e5443bbf5814783fabd6a1bcd2df08a9bec2ceb7` | 1280 | Firmware © Busicom/Intel; published by the Busicom 141-PF Replication Project for non-commercial use. Not committed here. |

Reference listing: *Busicom 141-PF Calculator, annotated disassembly rel. 1.0.1* (Busicom 141-PF Replication Project, CC BY-NC-SA 2.5), `$AI_HW_ENGINEER/system/busicom_141pf/spec/reference/Busicom-141PF-Calculator_asm_rel-1-0-1.txt`. Read for algorithms and data layout; nothing from it is copied into this MIT-licensed tree.

To place a copy here (gitignored):

```bash
cp "$AI_HW_ENGINEER/system/busicom_141pf/spec/reference/rom_141pf_combined.bin" original/
sha256sum original/rom_141pf_combined.bin
```
