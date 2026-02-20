/**
 * Decode RV32I instruction bits into human-readable fields (as executed).
 */

export interface InstrField {
  name: string;
  bits: string;
  hex: string;
  desc: string;
}

const REG_ABI = [
  "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1",
  "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7", "s2", "s3",
  "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
  "t5", "t6",
];

function regName(r: number): string {
  return r < 32 ? `x${r} (${REG_ABI[r]})` : `x${r}`;
}

function slice(instr: number, hi: number, lo: number): number {
  return (instr >>> lo) & ((1 << (hi - lo + 1)) - 1);
}

function sliceBin(instr: number, hi: number, lo: number): string {
  return slice(instr, hi, lo).toString(2).padStart(hi - lo + 1, "0");
}

export function decodeRv32i(instr: number): { mnemonic: string; fields: InstrField[] } {
  const opcode = slice(instr, 6, 0);
  const rd = slice(instr, 11, 7);
  const funct3 = slice(instr, 14, 12);
  const rs1 = slice(instr, 19, 15);
  const rs2 = slice(instr, 24, 20);
  const funct7 = slice(instr, 31, 25);

  switch (opcode) {
    case 0x13: {
      // I-type ALU
      const imm12 = (instr >> 20) & 0xfff;
      const imm = imm12 >= 0x800 ? imm12 - 0x1000 : imm12;
      const mnemonic = funct3 === 0 ? "addi" : `OP-IMM (f3=${funct3})`;
      return {
        mnemonic,
        fields: [
          { name: "imm[11:0]", bits: sliceBin(instr, 31, 20), hex: "0x" + imm12.toString(16), desc: `=${imm}` },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "I-type" },
        ],
      };
    }
    case 0x33: {
      const mnemonic =
        funct3 === 0 && funct7 === 0 ? "add" :
        funct3 === 0 && funct7 === 0x20 ? "sub" :
        `R-type (f3=${funct3} f7=${funct7})`;
      return {
        mnemonic,
        fields: [
          { name: "funct7", bits: sliceBin(instr, 31, 25), hex: "0x" + funct7.toString(16), desc: "" },
          { name: "rs2", bits: sliceBin(instr, 24, 20), hex: "0x" + rs2.toString(16), desc: regName(rs2) },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "R-type" },
        ],
      };
    }
    case 0x37: {
      const imm = instr & 0xfffff000;
      return {
        mnemonic: "lui",
        fields: [
          { name: "imm[31:12]", bits: sliceBin(instr, 31, 12), hex: "0x" + (imm >>> 12).toString(16), desc: `imm=${imm}` },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "U-type" },
        ],
      };
    }
    case 0x03: {
      const imm12 = (instr >> 20) & 0xfff;
      const imm = imm12 >= 0x800 ? imm12 - 0x1000 : imm12;
      const mnemonic = funct3 === 2 ? "lw" : `LOAD (f3=${funct3})`;
      return {
        mnemonic,
        fields: [
          { name: "imm[11:0]", bits: sliceBin(instr, 31, 20), hex: "0x" + imm12.toString(16), desc: `=${imm}` },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "I-type" },
        ],
      };
    }
    case 0x23: {
      const imm5 = slice(instr, 11, 7);
      const imm7 = slice(instr, 31, 25);
      const imm = ((imm7 << 5) | imm5) | (((instr >> 31) ? -1 : 0) << 12);
      const mnemonic = funct3 === 2 ? "sw" : `STORE (f3=${funct3})`;
      return {
        mnemonic,
        fields: [
          { name: "imm[11:5]", bits: sliceBin(instr, 31, 25), hex: "0x" + imm7.toString(16), desc: "" },
          { name: "rs2", bits: sliceBin(instr, 24, 20), hex: "0x" + rs2.toString(16), desc: regName(rs2) },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "imm[4:0]", bits: sliceBin(instr, 11, 7), hex: "0x" + imm5.toString(16), desc: `imm=${imm}` },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "S-type" },
        ],
      };
    }
    case 0x63: {
      const mnemonic = funct3 === 0 ? "beq" : funct3 === 1 ? "bne" : `BRANCH (f3=${funct3})`;
      return {
        mnemonic,
        fields: [
          { name: "imm", bits: sliceBin(instr, 31, 25) + sliceBin(instr, 11, 8) + sliceBin(instr, 7, 7), hex: "(B)", desc: "" },
          { name: "rs2", bits: sliceBin(instr, 24, 20), hex: "0x" + rs2.toString(16), desc: regName(rs2) },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "B-type" },
        ],
      };
    }
    case 0x6f: {
      return {
        mnemonic: "jal",
        fields: [
          { name: "imm", bits: "(J)", hex: "(J)", desc: "" },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "J-type" },
        ],
      };
    }
    case 0x67: {
      const imm12 = (instr >> 20) & 0xfff;
      const imm = imm12 >= 0x800 ? imm12 - 0x1000 : imm12;
      return {
        mnemonic: "jalr",
        fields: [
          { name: "imm[11:0]", bits: sliceBin(instr, 31, 20), hex: "0x" + imm12.toString(16), desc: `=${imm}` },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "I-type" },
        ],
      };
    }
    case 0x73: {
      const imm = (instr >> 20) & 0xfff;
      const mnemonic = imm === 0 ? "ecall" : `SYSTEM (imm=${imm})`;
      return {
        mnemonic,
        fields: [
          { name: "imm", bits: sliceBin(instr, 31, 20), hex: "0x" + imm.toString(16), desc: "" },
          { name: "rs1", bits: sliceBin(instr, 19, 15), hex: "0x" + rs1.toString(16), desc: regName(rs1) },
          { name: "funct3", bits: sliceBin(instr, 14, 12), hex: "0x" + funct3.toString(16), desc: "" },
          { name: "rd", bits: sliceBin(instr, 11, 7), hex: "0x" + rd.toString(16), desc: regName(rd) },
          { name: "opcode", bits: sliceBin(instr, 6, 0), hex: "0x" + opcode.toString(16), desc: "I-type" },
        ],
      };
    }
    case 0x00: {
      return {
        mnemonic: "nop",
        fields: [
          { name: "all", bits: "0".repeat(32), hex: "0x0", desc: "NOP" },
        ],
      };
    }
    default:
      return {
        mnemonic: `? (op=0x${opcode.toString(16)})`,
        fields: [
          { name: "raw", bits: (instr >>> 0).toString(2).padStart(32, "0"), hex: "0x" + (instr >>> 0).toString(16), desc: "" },
        ],
      };
  }
}
