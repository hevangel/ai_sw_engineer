# Contributing to AI Software Engineer

## The One Rule

**All changes to this repository are made by an AI coding agent.** Humans do not edit files directly — not code, not docs, not configuration. To change something, instruct your agent. Human-authored PRs are rejected.

Humans contribute by writing specifications, filing issues, reviewing PRs, and approving or rejecting changes.

## Workflow

1. Clone with submodules: `git clone --recurse-submodules https://github.com/<fork>/ai_sw_engineer.git`
2. Branch: `system/<name>`, `llvm/<chip>`, `docs/<topic>`, `fix/<desc>`, `infra/<desc>`.
3. Instruct the agent, e.g.:
   - "Reimplement Apple II Integer BASIC in Rust and run it on the 6502 RTL."
   - "Start the 4004 LLVM backend: data layout, register classes, and ADD/SUB selection."
   - "Capture an oracle transcript of the Busicom 141-PF manual examples."
4. The agent verifies (see `AGENTS.md` → Verification Before PR).
5. Open a PR with **What / Why / Verification / Agent**.

Example:

```
## What
Integer BASIC tokenizer and line editor in Rust; host replay of 12 oracle sessions.

## Why
First milestone of system/apple_ii/integer_basic.

## Verification
- cargo test: 12/12 transcripts identical
- Target: 3,912 bytes of 4,096-byte ROM budget (llvm-mos, -Oz)
- ai_hw_engineer mos_6502 RTL: 10/12 transcripts identical; 2 gaps documented in report

## Agent
Kiro (Claude) via Kiro CLI
```

## What we accept

- New software titles following `system/_template/software/`, with spec, plan, oracle and report.
- New backends following `llvm-_template/`, with ABI spec and lit/MC tests.
- Docs that are factually accurate and useful to an agent.
- Bug fixes with reproduction and re-verification.

## What we don't accept

- Human-authored file edits.
- Copyrighted ROM or disk images committed to git.
- Results without verification, or verification against the agent's own reading instead of the original.
- Edits inside `external/` submodules (patch in our own tree instead).

## License

Contributions are MIT-licensed (see [LICENSE](LICENSE)); third-party components keep their own licenses.
