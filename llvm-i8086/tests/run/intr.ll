; Interrupt handlers (spec/abi.md D5). See arith.ll for how this file is used.
; The test (i8086_codegen.rs: interrupt_handlers) runs `idle`, raises an interrupt
; whose vector points at `isr`, and checks that every register, including DS and
; ES, is the same after the handler returns, and that the handler ran with DS = CS.

target triple = "i8086"

@ticks = global i16 0
@mix = global i16 0
@wide = global i32 0

; Uses DX:AX, CX and more: whatever the handler does must not leak.
define i16 @scramble(i16 %x) noinline {
  %a = mul i16 %x, 31
  %b = xor i16 %a, 42405
  %c = shl i16 %x, 3
  %d = add i16 %b, %c
  %e = udiv i16 %d, 7
  %f = urem i16 %d, 5
  %g = add i16 %e, %f
  ret i16 %g
}

define void @isr() #0 {
  %t = load volatile i16, ptr @ticks
  %m = call i16 @scramble(i16 %t)
  store volatile i16 %m, ptr @mix
  %w = load volatile i32, ptr @wide
  %w2 = mul i32 %w, 65599
  %w3 = add i32 %w2, 12345
  store volatile i32 %w3, ptr @wide
  %n = add i16 %t, 1
  store volatile i16 %n, ptr @ticks
  ret void
}

; The same through the calling convention rustc uses for extern "msp430-interrupt".
define msp430_intrcc void @isr_cc() {
  %t = load volatile i16, ptr @ticks
  %n = add i16 %t, 100
  store volatile i16 %n, ptr @ticks
  ret void
}

; The interrupted program: no memory access, so it works with any DS.
define void @idle() {
entry:
  br label %loop
loop:
  br label %loop
}

attributes #0 = { "interrupt" }
