#!/usr/bin/env python3
"""Adds the `i8086-unknown-none` target to a rust-lang/rust checkout (commit 7df81381d, the
nightly-2026-10-15 source) so that rustc links our LLVM with the I8086 backend:

    apply_i8086_target.py <rust source dir>

It is idempotent. Each edit mirrors how msp430 is wired in; the compiler's exhaustive
matches name every place an architecture is listed."""
import re
import sys

root = sys.argv[1].rstrip('/')


def edit(path, old, new, count=1):
    p = f'{root}/{path}'
    s = open(p, encoding='utf8').read()
    if new in s:
        return
    assert old in s, (path, old[:60])
    open(p, 'w', encoding='utf8').write(s.replace(old, new, count))


T = 'compiler/rustc_target/src/'
# --- the architecture
edit(T + 'spec/mod.rs', '        Hexagon = "hexagon",\n', '        Hexagon = "hexagon",\n        I8086 = "i8086",\n')
edit(T + 'spec/mod.rs', '            Self::Hexagon => sym::hexagon,\n', '            Self::Hexagon => sym::i8086_arch_placeholder,\n'.replace('i8086_arch_placeholder', 'hexagon') + '            Self::I8086 => sym::i8086,\n')
edit(T + 'spec/mod.rs', 'AmdGpu | Arm | Arm64EC | Avr | Bpf | CSky | Hexagon | LoongArch32', 'AmdGpu | Arm | Arm64EC | Avr | Bpf | CSky | Hexagon | I8086 | LoongArch32')
edit(T + 'spec/mod.rs', '            Sparc | Avr | M68k | Msp430 => {', '            Sparc | Avr | I8086 | M68k | Msp430 => {')
edit(T + 'spec/mod.rs', '            Arch::Msp430 => (Architecture::Msp430, None),\n', '            Arch::Msp430 => (Architecture::Msp430, None),\n            Arch::I8086 => return None,\n')
edit(T + 'spec/mod.rs', '    ("msp430-none-elf", msp430_none_elf),\n', '    ("msp430-none-elf", msp430_none_elf),\n    ("i8086-unknown-none", i8086_unknown_none),\n')
edit('compiler/rustc_span/src/symbol.rs', '        i128_type,\n', '        i128_type,\n        i8086,\n')
open(f'{root}/{T}spec/targets/i8086_unknown_none.rs', 'w', encoding='utf8').write('''use crate::spec::{
    Arch, Cc, LinkerFlavor, Lld, PanicStrategy, RelocModel, Target, TargetMetadata, TargetOptions,
};

// llvm-i8086 (https://github.com/hevangel/ai_sw_engineer, llvm-i8086/spec/abi.md D1): the Intel
// 8086/8088 in real mode, tiny model (one 64 KiB segment, 16-bit near pointers). Address space 1
// is the 32-bit far data pointer (segment in the high word).
pub(crate) fn target() -> Target {
    Target {
        llvm_target: "i8086-unknown-none".into(),
        metadata: TargetMetadata {
            description: Some("Intel 8086/8088, real mode, tiny model".into()),
            tier: Some(3),
            host_tools: Some(false),
            std: Some(false),
        },
        pointer_width: 16,
        data_layout: "e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16"
            .into(),
        arch: Arch::I8086,

        options: TargetOptions {
            c_int_width: 16,
            linker: Some("i8086-ld".into()),
            linker_flavor: LinkerFlavor::Gnu(Cc::No, Lld::No),

            // no atomics: one core, interrupts are the only concurrency
            max_atomic_width: Some(0),
            atomic_cas: false,

            panic_strategy: PanicStrategy::Abort,
            relocation_model: RelocModel::Static,
            disable_redzone: true,
            default_codegen_units: Some(1),
            trap_unreachable: false,
            emit_debug_gdb_scripts: false,
            eh_frame_header: false,

            ..Default::default()
        },
    }
}
''')
# --- the calling convention: as msp430 (small aggregates in registers, the rest by reference)
edit(T + 'callconv/mod.rs', '            Arch::Msp430 => msp430::compute_abi_info(cx, self),\n', '            Arch::Msp430 | Arch::I8086 => msp430::compute_abi_info(cx, self),\n')
# --- target features: none
edit(T + 'target_features.rs', 'Arch::AmdGpu | Arch::Msp430 | Arch::SpirV | Arch::Other(_) => &[],', 'Arch::AmdGpu | Arch::I8086 | Arch::Msp430 | Arch::SpirV | Arch::Other(_) => &[],')
edit(T + 'target_features.rs', 'Arch::Msp430 | Arch::SpirV | Arch::Xtensa | Arch::Other(_) => &[],', 'Arch::I8086 | Arch::Msp430 | Arch::SpirV | Arch::Xtensa | Arch::Other(_) => &[],')
# --- the LLVM target
edit('compiler/rustc_llvm/build.rs', '    "msp430",\n', '    "msp430",\n    "i8086",\n')
edit('compiler/rustc_llvm/src/lib.rs', '''    init_target!(
        llvm_component = "msp430",''', '''    init_target!(
        llvm_component = "i8086",
        LLVMInitializeI8086TargetInfo,
        LLVMInitializeI8086Target,
        LLVMInitializeI8086TargetMC,
        LLVMInitializeI8086AsmPrinter,
        LLVMInitializeI8086AsmParser
    );
    init_target!(
        llvm_component = "msp430",''')

# --- interrupt handlers: `extern "msp430-interrupt"` is also the interrupt ABI of i8086 (the
# LLVM calling convention it selects is the one llvm-i8086 compiles as an interrupt handler,
# spec/abi.md D5)
edit(T + 'spec/abi_map.rs', '            Arch::Msp430 => ArchKind::Msp430,\n', '            Arch::Msp430 => ArchKind::Msp430,\n            Arch::I8086 => ArchKind::I8086,\n')
edit(T + 'spec/abi_map.rs', '    LoongArch,\n    Msp430,\n', '    I8086,\n    LoongArch,\n    Msp430,\n')
edit(T + 'spec/abi_map.rs', '(ExternAbi::Msp430Interrupt, ArchKind::Msp430) => {', '(ExternAbi::Msp430Interrupt, ArchKind::Msp430 | ArchKind::I8086) => {')

# --- no inline assembly on i8086 (the HAL is in assembly files)
edit(T + 'asm/mod.rs', '            Arch::Xtensa => Some(Self::Xtensa),\n            Arch::Other(_) => None,', '            Arch::Xtensa => Some(Self::Xtensa),\n            Arch::I8086 => None,\n            Arch::Other(_) => None,')

# --- c-variadic: the LLVM va_arg of llvm-i8086 (spec/abi.md D3a)
edit('compiler/rustc_codegen_llvm/src/va_arg.rs', '        Arch::Avr | Arch::M68k | Arch::Msp430 => {', '        Arch::Avr | Arch::I8086 | Arch::M68k | Arch::Msp430 => {')

# --- no 128-bit floating point: the backend has no fp128 soft-float libcalls (abi.md D6)
edit('compiler/rustc_codegen_llvm/src/llvm_util.rs', '''    cfg.has_reliable_f128 = match (target_arch, target_os) {
''', '''    cfg.has_reliable_f128 = match (target_arch, target_os) {
        (Arch::I8086, _) => false,
''')
