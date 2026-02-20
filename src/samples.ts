export const SAMPLE_REGISTERS = `# Register updates
_start:
  addi x1, x0, 42
  addi x2, x0, 10
  add  x3, x1, x2
  sub  x4, x1, x2
  ecall
`;

export const SAMPLE_MEMORY = `# Memory read/write
_start:
  lui  x1, 0x10000
  addi x2, x0, 0x42
  sw   x2, 0(x1)
  lw   x3, 0(x1)
  ecall
`;

export const SAMPLE_BRANCH = `# Branch and jump
_start:
  addi x1, x0, 5
  addi x2, x0, 5
  beq  x1, x2, equal
  addi x3, x0, 1
  j    done
equal:
  addi x3, x0, 2
done:
  ecall
`;

export const SAMPLE_READ_PRINT = `# Read input via ecall, then print it
# a7=5: read int, a7=13: read char, a7=11: print int, a7=12: print char
_start:
  addi a7, x0, 5      # read integer
  ecall               # waits for input -> a0
  addi a7, x0, 11     # print integer
  ecall
  addi a0, x0, 10     # newline
  addi a7, x0, 12
  ecall
  addi a7, x0, 10     # exit
  ecall
`;

export const SAMPLE_PRINT = `# Print output via ecall
# a7=11: print int (a0), a7=12: print char (a0), a7=10: exit
_start:
  addi a0, x0, 72
  addi a7, x0, 12
  ecall
  addi a0, x0, 105
  ecall
  addi a0, x0, 10
  ecall
  addi a0, x0, 42
  addi a7, x0, 11
  ecall
  addi a7, x0, 10
  ecall
`;

export const SAMPLE_FULL = `# Full demo: registers, memory, branch, halt
_start:
  addi x1, x0, 10
  addi x2, x0, 20
  add  x3, x1, x2
  lui  x4, 0x08
  sw   x3, 0(x4)
  lw   x5, 0(x4)
  beq  x3, x5, ok
  addi x6, x0, 1
  j    end
ok:
  addi x6, x0, 0
end:
  ecall
`;

// LC-3 samples
export const SAMPLE_LC3_SIMPLE = `; LC-3: Add two numbers
.ORIG x3000
_start:
  ADD R1, R1, #10
  ADD R2, R2, #20
  ADD R3, R1, R2
  HALT
`;

export const SAMPLE_LC3_BRANCH = `; LC-3: Branch example
.ORIG x3000
_start:
  ADD R1, R1, #5
  ADD R2, R2, #5
  NOT R3, R1
  ADD R3, R3, #1
  ADD R3, R3, R2
  BRz equal
  ADD R4, R4, #1
  BRnzp done
equal:
  ADD R4, R4, #2
done:
  HALT
`;

export const SAMPLE_LC3_FULL = `; LC-3: ADD, AND, NOT, BR, LEA, LD, ST, LDR, STR, JSR, JSRR, JMP, .FILL, .BLKW
.ORIG x3000
_start:
  ADD  R1, R1, #10
  ADD  R2, R2, #20
  AND  R3, R1, R2
  NOT  R4, R3
  LEA  R5, data
  LD   R6, data
  ST   R6, store1
  LDR  R0, R5, #1
  STR  R0, R5, #2
  ADD  R1, R1, #-5
  BRn  neg
  BRz  zero
  BRp  pos
neg:
  ADD  R2, R2, #1
  BRnzp after
zero:
  ADD  R2, R2, #2
  BRnzp after
pos:
  ADD  R2, R2, #3
after:
  JSR  subroutine
  HALT
subroutine:
  ADD  R6, R6, #1
  JSRR R7
data:
  .FILL x0048
  .FILL x0069
store1:
  .BLKW 1
  HALT
`;

// MIPS samples
export const SAMPLE_MIPS_SIMPLE = `# MIPS: Add two numbers
_start:
  addi $t0, $zero, 10
  addi $t1, $zero, 20
  add  $t2, $t0, $t1
  addi $v0, $zero, 10
  syscall
`;

export const SAMPLE_MIPS_BRANCH = `# MIPS: Branch example
_start:
  addi $t0, $zero, 5
  addi $t1, $zero, 5
  beq  $t0, $t1, equal
  addi $t2, $zero, 1
  j    done
equal:
  addi $t2, $zero, 2
done:
  addi $v0, $zero, 10
  syscall
`;

export const SAMPLE_MIPS_FULL = `# MIPS: add, sub, and, or, addi, lw, sw, beq, bne, j, jal, jr, syscall
# Base 0x1000 (li sign-extends; 0x1000 fits in 16-bit)
_start:
  addi $t0, $zero, 0x1000
  addi $t1, $zero, 10
  addi $t2, $zero, 20
  add  $t3, $t1, $t2
  sub  $t4, $t2, $t1
  and  $t5, $t1, $t2
  or   $t6, $t1, $t2
  sw   $t3, 0($t0)
  sw   $t4, 4($t0)
  lw   $t7, 0($t0)
  lw   $s0, 4($t0)
  beq  $t3, $t7, eq_label
  addi $s1, $zero, 1
  j    next
eq_label:
  addi $s1, $zero, 0
next:
  bne  $t1, $t2, ne_label
  j    after_ne
ne_label:
  addi $s2, $s2, 1
after_ne:
  addi $a0, $zero, 72
  addi $v0, $zero, 11
  syscall
  addi $a0, $zero, 105
  syscall
  addi $a0, $zero, 10
  syscall
  addi $a0, $zero, 42
  addi $v0, $zero, 1
  syscall
  jal  my_func
  addi $v0, $zero, 10
  syscall
my_func:
  addi $s3, $s3, 100
  jr   $ra
`;

export function getDefaultSample(arch: string): string {
  switch (arch) {
    case "LC3":
      return SAMPLE_LC3_FULL;
    case "MIPS":
      return SAMPLE_MIPS_FULL;
    default:
      return SAMPLE_FULL;
  }
}
