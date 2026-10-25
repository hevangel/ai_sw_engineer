; Execution tests (system/ibm_pc/emulator/tests/i8086_codegen.rs): the object
; built from this file is linked and called on the 8088 core, and each result
; is compared with the same computation on the host. Not a lit test: the
; extension is .ll but lit skips this directory (see lit.cfg.py).

target triple = "i8086"

define i16 @add16(i16 %a, i16 %b) {
  %r = add i16 %a, %b
  ret i16 %r
}

define i16 @sub16(i16 %a, i16 %b) {
  %r = sub i16 %a, %b
  ret i16 %r
}

define i16 @and16(i16 %a, i16 %b) {
  %r = and i16 %a, %b
  ret i16 %r
}

define i16 @or16(i16 %a, i16 %b) {
  %r = or i16 %a, %b
  ret i16 %r
}

define i16 @xor16(i16 %a, i16 %b) {
  %r = xor i16 %a, %b
  ret i16 %r
}

define i16 @neg16(i16 %a) {
  %r = sub i16 0, %a
  ret i16 %r
}

define i16 @not16(i16 %a) {
  %r = xor i16 %a, -1
  ret i16 %r
}

define i16 @inc16(i16 %a) {
  %r = add i16 %a, 1
  ret i16 %r
}

define i16 @add_imm16(i16 %a) {
  %r = add i16 %a, 1000
  ret i16 %r
}

define i16 @sub_imm16(i16 %a) {
  %r = sub i16 %a, 5
  ret i16 %r
}

define i16 @and_imm16(i16 %a) {
  %r = and i16 %a, 4080
  ret i16 %r
}

define zeroext i8 @add8(i8 zeroext %a, i8 zeroext %b) {
  %r = add i8 %a, %b
  ret i8 %r
}

define zeroext i8 @sub8(i8 zeroext %a, i8 zeroext %b) {
  %r = sub i8 %a, %b
  ret i8 %r
}

define zeroext i8 @xor8(i8 zeroext %a, i8 zeroext %b) {
  %r = xor i8 %a, %b
  ret i8 %r
}
