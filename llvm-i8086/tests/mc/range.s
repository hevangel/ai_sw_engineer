# RUN: not llvm-mc -triple=i8086 -filetype=obj %s -o /dev/null 2>&1 | FileCheck %s
#
# loop and jcxz have only a short form and do not relax, so a target out of
# range is an error rather than a silently wrong displacement.
  loop far_away
  jcxz far_away
  .space 300
far_away:
# CHECK: error: branch target out of range for a short jump
# CHECK: error: branch target out of range for a short jump
