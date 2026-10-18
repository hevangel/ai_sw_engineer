# Failure notes

Post-mortems for bugs that **escaped verification**: found by the original program's behavior, a second emulator, a reviewer or real use — not by the title's or backend's own tests.

File name: `YYYY-MM-DD-<area>-<short-name>.md`.

## Template

```markdown
# <title>

- Area: system/<system>/<software> | llvm-<chip> | platform
- Found by: <external oracle>

## What broke
## Root cause — and why our tests missed it
## Fix and re-verification
## Prevention (rule/test added; link the AGENTS.md change if any)
```

Blame-free: the question is which step let a shared assumption pass as verified. Note in particular whether the bug was in the Rust reimplementation, the LLVM backend, the platform runtime, or the simulator.

## Index

_None yet._
