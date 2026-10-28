# Learning from escaped defects

Read this when a bug is found by anything other than the title's own tests, and when planning verification for a new title or backend.

A bug found by anything other than the title's own tests (the original program's behavior, a second emulator, a reviewer) is an escaped defect: write a post-mortem in `failure_notes/`.

- **Oracle from the original**: expected behavior comes from running the original binary or from the manufacturer's documentation, never from re-reading your own Rust.
- **Two-stage check**: host pass + target pass are both required; a host pass with a target fail points at the backend first.
- **Assumption ledger**: behavior the documentation does not state gets an `ASSUMPTION:` comment in code and an entry in `spec/spec.md`.
- **Whole-program size gate from the first function that lowers**: link-and-measure the whole title against the ROM budget (per-function `.text` sum and largest function) in the backend's test runner, long before linker or relaxation polish. Component tests being green says nothing about whether the whole fits.
