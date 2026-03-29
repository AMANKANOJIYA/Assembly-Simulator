//! RISC-V RV32I full base assembler and executor.
//! Supports: LUI, AUIPC, JAL, JALR, BEQ/BNE/BLT/BGE/BLTU/BGEU, LB/LH/LW/LBU/LHU, SB/SH/SW,
//! ADDI, SLTI, SLTIU, XORI, ORI, ANDI, SLLI, SRLI, SRAI, ADD, SUB, SLT, SLTU, XOR, OR, AND, SLL, SRL, SRA,
//! ECALL, EBREAK; pseudo: NOP, LI, MV, RET, J.

use crate::memory::Memory;
use crate::plugin::*;
use std::collections::HashMap;
use std::str::FromStr;

pub struct Rv32iPlugin;

impl Rv32iPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Rv32iPlugin {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_reg(name: &str) -> Option<u32> {
    let name = name.trim().trim_end_matches(',').to_lowercase();
    if name.starts_with('x') {
        u32::from_str(name.trim_start_matches('x')).ok().filter(|&r| r < 32)
    } else if name == "zero" || name == "x0" {
        Some(0)
    } else {
        // ABI names for x1-x31
        let abi: &[(&str, u32)] = &[
            ("ra", 1), ("sp", 2), ("gp", 3), ("tp", 4),
            ("t0", 5), ("t1", 6), ("t2", 7), ("s0", 8), ("s1", 9),
            ("a0", 10), ("a1", 11), ("a2", 12), ("a3", 13), ("a4", 14),
            ("a5", 15), ("a6", 16), ("a7", 17), ("s2", 18), ("s3", 19),
            ("s4", 20), ("s5", 21), ("s6", 22), ("s7", 23), ("s8", 24),
            ("s9", 25), ("s10", 26), ("s11", 27), ("t3", 28), ("t4", 29),
            ("t5", 30), ("t6", 31),
        ];
        abi.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
    }
}

fn encode_r(opcode: u32, rd: u32, funct3: u32, rs1: u32, rs2: u32, funct7: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

fn encode_i(opcode: u32, rd: u32, funct3: u32, rs1: u32, imm: i32) -> u32 {
    let imm = (imm as u32) & 0xFFF;
    (imm << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

fn encode_s(opcode: u32, imm: i32, funct3: u32, rs1: u32, rs2: u32) -> u32 {
    let imm = (imm as u32) & 0xFFF;
    let imm11_5 = (imm >> 5) & 0x7F;
    let imm4_0 = imm & 0x1F;
    (imm11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm4_0 << 7) | opcode
}

fn encode_b(opcode: u32, imm: i32, funct3: u32, rs1: u32, rs2: u32) -> u32 {
    let imm = (imm as u32) & 0x1FFF;
    let imm12 = (imm >> 12) & 1;
    let imm11 = (imm >> 11) & 1;
    let imm10_5 = (imm >> 5) & 0x3F;
    let imm4_1 = (imm >> 1) & 0xF;
    (imm12 << 31) | (imm10_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm11 << 11) | (imm4_1 << 7) | opcode
}

fn encode_u(opcode: u32, rd: u32, imm: u32) -> u32 {
    ((imm & 0xFFFFF) << 12) | (rd << 7) | opcode
}

fn encode_j(opcode: u32, rd: u32, imm: i32) -> u32 {
    let imm = (imm as u32) & 0x1FFFFF;
    let imm20 = (imm >> 20) & 1;
    let imm10_1 = (imm >> 1) & 0x3FF;
    let imm11 = (imm >> 11) & 1;
    let imm19_12 = (imm >> 12) & 0xFF;
    (imm20 << 31) | (imm19_12 << 12) | (imm11 << 20) | (imm10_1 << 21) | (rd << 7) | opcode
}

/// 5-stage pipeline with stage-specific actions
fn pipeline_5(
    instr: Option<u32>,
    fetch: &str,
    decode: &str,
    execute: &str,
    mem: &str,
    wb: &str,
) -> Vec<PipelineCycleInfo> {
    vec![
        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: instr, action: fetch.into() },
        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: instr, action: decode.into() },
        PipelineCycleInfo { stage: "Execute".into(), instruction_bits: instr, action: execute.into() },
        PipelineCycleInfo { stage: "Memory".into(), instruction_bits: instr, action: mem.into() },
        PipelineCycleInfo { stage: "Write-back".into(), instruction_bits: instr, action: wb.into() },
    ]
}

fn pipeline_halt(instr: Option<u32>, fetch: &str, decode: &str, halted: &str) -> Vec<PipelineCycleInfo> {
    vec![
        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: instr, action: fetch.into() },
        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: instr, action: decode.into() },
        PipelineCycleInfo { stage: "Halted".into(), instruction_bits: instr, action: halted.into() },
    ]
}

fn pipeline_empty() -> Vec<PipelineCycleInfo> {
    vec![]
}

impl ArchitecturePlugin for Rv32iPlugin {
    fn name(&self) -> &str {
        "RV32I"
    }

    fn assemble(&self, source: &str) -> ProgramImage {
        let mut bytes = Vec::new();
        let mut source_map = Vec::new();
        let mut errors = Vec::new();
        let mut labels: HashMap<String, u32> = HashMap::new();
        let mut pending_refs: Vec<(usize, String, u32)> = Vec::new();

        let lines: Vec<&str> = source.lines().collect();
        let mut pc: u32 = 0;

        // First pass: collect labels and assemble
        for (line_no, line) in lines.iter().enumerate() {
            let line_num = (line_no + 1) as u32;
            let line = line
                .split("//")
                .next()
                .unwrap_or(line)
                .split('#')
                .next()
                .unwrap_or(line)
                .trim();
            if line.is_empty() {
                continue;
            }

            let col = (line.find(|c: char| !c.is_whitespace()).unwrap_or(0) + 1) as u32;

            // Check for label
            if let Some(idx) = line.find(':') {
                let label = line[..idx].trim().to_string();
                if !label.is_empty() && !label.starts_with('.') {
                    labels.insert(label, pc);
                }
                let rest = line[idx + 1..].trim();
                if rest.is_empty() {
                    continue;
                }
                // Parse instruction on same line
                if let Err(e) = parse_instruction(
                    rest,
                    line_num,
                    col,
                    pc,
                    &labels,
                    &mut bytes,
                    &mut source_map,
                    &mut errors,
                    &mut pending_refs,
                ) {
                    errors.push(e);
                }
                pc += 4;
                continue;
            }

            if let Err(e) = parse_instruction(
                line,
                line_num,
                col,
                pc,
                &labels,
                &mut bytes,
                &mut source_map,
                &mut errors,
                &mut pending_refs,
            ) {
                errors.push(e);
            }
            pc += 4;
        }

        // Resolve pending refs (RISC-V offsets are in halfwords)
        for (offset, label, line_num) in pending_refs {
            if let Some(&target) = labels.get(&label) {
                let rel = (target as i32 - offset as i32) / 2;
                let insn = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                let new_insn = patch_branch_or_jal(insn, rel);
                bytes[offset..offset + 4].copy_from_slice(&new_insn.to_le_bytes());
            } else {
                errors.push(AssemblerError {
                    line: line_num,
                    column: 1,
                    message: format!("Unknown label: {}", label),
                });
            }
        }

        ProgramImage {
            entry_pc: labels.get("_start").copied().unwrap_or(0),
            bytes,
            source_map,
            errors,
        }
    }

    fn reset(&self, _config: &ResetConfig) -> CpuState {
        CpuState {
            pc: 0,
            regs: vec![0u32; 32],
            halted: false,
        }
    }

    fn step(&self, state: &CpuState, memory: &[u8], _mode: StepMode, input: Option<&str>) -> StepResult {
        if state.halted {
            return StepResult {
                new_state: state.clone(),
                events: vec![TraceEvent::Halted],
                undo_log: vec![],
                cycles_added: 0,
                halted: true,
                error: None,
                instruction_bits: None,
                pipeline_stages: pipeline_empty(),
                io_output: None,
                io_input_requested: None,
            };
        }

        let mut mem = Memory::new(memory.len());
        mem.data_mut().copy_from_slice(memory);

        let pc = state.pc;
        let mut regs = state.regs.clone();

        let instr = match mem.read_u32_le(pc) {
            Ok(i) => i,
            Err(e) => {
                return StepResult {
                    new_state: state.clone(),
                    events: vec![],
                    undo_log: vec![],
                    cycles_added: 0,
                    halted: false,
                    error: Some(e),
                    instruction_bits: None,
                    pipeline_stages: pipeline_empty(),
                    io_output: None,
                io_input_requested: None,
                };
            }
        };

        let mut undo_log = Vec::new();
        let mut events = vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Alu];

        let opcode = instr & 0x7F;
        let rd = ((instr >> 7) & 0x1F) as usize;
        let funct3 = (instr >> 12) & 0x7;
        let rs1 = ((instr >> 15) & 0x1F) as usize;
        let rs2 = ((instr >> 20) & 0x1F) as usize;
        let funct7 = (instr >> 25) & 0x7F;

        let imm_i = ((instr as i32) >> 20) as u32;
        let imm_s = (((instr >> 7) & 0x1F) | ((instr >> 25) << 5)) as i32;
        let imm_b = {
            let b11 = (instr >> 7) & 1;
            let b4_1 = (instr >> 8) & 0xF;
            let b10_5 = (instr >> 25) & 0x3F;
            let b12 = (instr >> 31) & 1;
            let imm = (b12 << 12) | (b11 << 11) | (b10_5 << 5) | (b4_1 << 1);
            ((imm as i32) << 19) >> 19
        };
        let imm_u = instr & 0xFFFFF000;
        let imm_j = {
            let _imm = (instr >> 12) & 0xFFFFF;
            let _sign = (instr >> 31) & 1;
            let i20 = (instr >> 31) & 1;
            let i10_1 = (instr >> 21) & 0x3FF;
            let i11 = (instr >> 20) & 1;
            let i19_12 = (instr >> 12) & 0xFF;
            let imm = (i20 << 20) | (i19_12 << 12) | (i11 << 11) | (i10_1 << 1);
            ((imm as i32) << 11) >> 11
        };

        let next_pc = pc + 4;

        let pipeline_stages: Vec<PipelineCycleInfo>;
        match opcode {
            0x13 => {
                // I-type ALU: addi, slti, sltiu, xori, ori, andi, slli, srli, srai
                let rs1_val = regs[rs1];
                let imm = imm_i as i32;
                let imm_u = imm_i;
                let shamt = (imm_i & 0x1F) as u32;
                let result = match funct3 {
                    0 => (rs1_val as i32).wrapping_add(imm) as u32, // addi
                    2 => if (rs1_val as i32) < imm { 1 } else { 0 }, // slti
                    3 => if rs1_val < imm_u { 1 } else { 0 },       // sltiu
                    4 => rs1_val ^ imm_u,                           // xori
                    6 => rs1_val | imm_u,                           // ori
                    7 => rs1_val & imm_u,                           // andi
                    1 => rs1_val << shamt,                          // slli
                    5 => match funct7 {
                        0x00 => rs1_val >> shamt,                   // srli
                        0x20 => ((rs1_val as i32) >> shamt) as u32, // srai
                        _ => {
                            return StepResult {
                                new_state: state.clone(),
                                events: vec![],
                                undo_log: vec![],
                                cycles_added: 0,
                                halted: false,
                                error: Some(format!("Unknown I-type funct7: {}", funct7)),
                                instruction_bits: Some(instr),
                                pipeline_stages: pipeline_empty(),
                                io_output: None,
                io_input_requested: None,
                            };
                        }
                    },
                    _ => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(format!("Unknown I-type funct3: {}", funct3)),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                if rd > 0 {
                    undo_log.push(UndoEntry::RegWrite {
                        reg: rd,
                        old_value: regs[rd],
                        new_value: result,
                    });
                    regs[rd] = result;
                }
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next_pc,
                });
                events.push(TraceEvent::RegWrite);
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rd=x{}, rs1=x{}, imm={}", rd, rs1, imm_i),
                    &format!("ALU: x{} op imm = {}", rs1, result),
                    "NOP (no memory access)",
                    &format!("x{} ← {}", rd, result),
                );
            }
            0x33 => {
                // R-type: add, sub, slt, sltu, xor, or, and, sll, srl, sra
                let a = regs[rs1];
                let b = regs[rs2];
                let shamt = (b & 0x1F) as u32;
                let result = match (funct3, funct7) {
                    (0, 0x00) => a.wrapping_add(b),                      // add
                    (0, 0x20) => a.wrapping_sub(b),                      // sub
                    (2, 0x00) => if (a as i32) < (b as i32) { 1 } else { 0 }, // slt
                    (3, 0x00) => if a < b { 1 } else { 0 },              // sltu
                    (4, 0x00) => a ^ b,                                   // xor
                    (6, 0x00) => a | b,                                   // or
                    (7, 0x00) => a & b,                                   // and
                    (1, 0x00) => a << shamt,                              // sll
                    (5, 0x00) => a >> shamt,                              // srl
                    (5, 0x20) => ((a as i32) >> shamt) as u32,            // sra
                    _ => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(format!("Unknown R-type: funct3={} funct7={}", funct3, funct7)),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                if rd > 0 {
                    undo_log.push(UndoEntry::RegWrite {
                        reg: rd,
                        old_value: regs[rd],
                        new_value: result,
                    });
                    regs[rd] = result;
                }
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next_pc,
                });
                events.push(TraceEvent::RegWrite);
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rd=x{}, rs1=x{}, rs2=x{}", rd, rs1, rs2),
                    &format!("ALU: x{} op x{} = {}", rs1, rs2, result),
                    "NOP (no memory access)",
                    &format!("x{} ← {}", rd, result),
                );
            }
            0x37 => {
                // lui
                if rd > 0 {
                    undo_log.push(UndoEntry::RegWrite {
                        reg: rd,
                        old_value: regs[rd],
                        new_value: imm_u,
                    });
                    regs[rd] = imm_u;
                }
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next_pc,
                });
                events.push(TraceEvent::RegWrite);
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rd=x{}, imm[31:12]=0x{:05X}", rd, imm_u >> 12),
                    &format!("Execute: imm << 12 = 0x{:08X}", imm_u),
                    "NOP (no memory access)",
                    &format!("x{} ← 0x{:08X}", rd, imm_u),
                );
            }
            0x17 => {
                // auipc: rd = PC + (imm_u)
                let result = pc.wrapping_add(imm_u);
                if rd > 0 {
                    undo_log.push(UndoEntry::RegWrite {
                        reg: rd,
                        old_value: regs[rd],
                        new_value: result,
                    });
                    regs[rd] = result;
                }
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next_pc,
                });
                events.push(TraceEvent::RegWrite);
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rd=x{}, imm[31:12]=0x{:05X}", rd, imm_u >> 12),
                    &format!("Execute: PC + 0x{:08X} = 0x{:08X}", imm_u, result),
                    "NOP (no memory access)",
                    &format!("x{} ← 0x{:08X}", rd, result),
                );
            }
            0x03 => {
                // lb, lh, lw, lbu, lhu
                events.push(TraceEvent::Mem);
                let addr = regs[rs1].wrapping_add(imm_i);
                let load_result = match funct3 {
                    0 => mem.read_u8(addr).map(|b| (b as i8) as u32),   // lb
                    1 => mem.read_u16_le(addr).map(|h| (h as i16) as u32), // lh
                    2 => mem.read_u32_le(addr),                          // lw
                    4 => mem.read_u8(addr).map(|b| b as u32),            // lbu
                    5 => mem.read_u16_le(addr).map(|h| h as u32),        // lhu
                    _ => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(format!("Unknown load funct3: {}", funct3)),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                match load_result {
                    Ok(val) => {
                        if rd > 0 {
                            undo_log.push(UndoEntry::RegWrite {
                                reg: rd,
                                old_value: regs[rd],
                                new_value: val,
                            });
                            regs[rd] = val;
                        }
                        undo_log.push(UndoEntry::Pc {
                            old_value: pc,
                            new_value: next_pc,
                        });
                        events.push(TraceEvent::RegWrite);
                        pipeline_stages = pipeline_5(
                            Some(instr),
                            &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                            &format!("Extract rd=x{}, rs1=x{}, imm={}", rd, rs1, imm_i as i32),
                            &format!("Execute: addr = x{} + {} = 0x{:08X}", rs1, imm_i as i32, addr),
                            &format!("Memory: load word from Mem[0x{:08X}] = {}", addr, val),
                            &format!("x{} ← {}", rd, val),
                        );
                    }
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                }
            }
            0x23 => {
                // sb, sh, sw
                events.push(TraceEvent::Mem);
                let addr = regs[rs1].wrapping_add(imm_s as u32);
                let val = regs[rs2];
                let old_bytes: Result<Vec<u8>, _> = match funct3 {
                    0 => mem.read_u8(addr).map(|b| vec![b]),
                    1 => mem.read_u16_le(addr).map(|h| h.to_le_bytes().to_vec()),
                    2 => mem.read_u32_le(addr).map(|w| w.to_le_bytes().to_vec()),
                    _ => Err("Unknown store funct3".to_string()),
                };
                let write_result: Result<(), String> = match funct3 {
                    0 => mem.write_u8(addr, (val & 0xFF) as u8).map(|_| ()),
                    1 => mem.write_u16_le(addr, (val & 0xFFFF) as u16).map(|_| ()),
                    2 => mem.write_u32_le(addr, val).map(|_| ()),
                    _ => Err(format!("Unknown store funct3: {}", funct3)),
                };
                match (old_bytes, write_result) {
                    (Err(e), _) | (_, Err(e)) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                    (Ok(old), Ok(())) => {
                        let new_bytes: Vec<u8> = match funct3 {
                            0 => vec![(val & 0xFF) as u8],
                            1 => (val & 0xFFFF).to_le_bytes().to_vec(),
                            _ => val.to_le_bytes().to_vec(),
                        };
                        for (i, &new_b) in new_bytes.iter().enumerate() {
                            let old_b = old.get(i).copied().unwrap_or(0);
                            undo_log.push(UndoEntry::MemWrite {
                                addr: addr + i as u32,
                                old_value: old_b,
                                new_value: new_b,
                            });
                        }
                        undo_log.push(UndoEntry::Pc {
                            old_value: pc,
                            new_value: next_pc,
                        });
                        pipeline_stages = pipeline_5(
                            Some(instr),
                            &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                            &format!("Extract rs1=x{}, rs2=x{}, imm={}", rs1, rs2, imm_s),
                            &format!("Execute: addr = x{} + {} = 0x{:08X}", rs1, imm_s, addr),
                            &format!("Memory: store {} to Mem[0x{:08X}]", val, addr),
                            "NOP (no register write)",
                        );
                    }
                }
            }
            0x63 => {
                // beq, bne, blt, bge, bltu, bgeu
                let a = regs[rs1];
                let b = regs[rs2];
                let (take_branch, cmp_str) = match funct3 {
                    0 => (a == b, "beq"),
                    1 => (a != b, "bne"),
                    4 => ((a as i32) < (b as i32), "blt"),
                    5 => ((a as i32) >= (b as i32), "bge"),
                    6 => (a < b, "bltu"),
                    7 => (a >= b, "bgeu"),
                    _ => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(format!("Unknown B-type funct3: {}", funct3)),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                let target = pc.wrapping_add(imm_b as u32);
                let next = if take_branch { target } else { next_pc };
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next,
                });
                let exec_str = if take_branch {
                    format!("{}: {} == {} → taken, PC ← 0x{:08X}", cmp_str, a, b, target)
                } else {
                    format!("{}: {} != {} → not taken, PC ← 0x{:08X}", cmp_str, a, b, next_pc)
                };
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rs1=x{}, rs2=x{}, imm={}", rs1, rs2, imm_b),
                    &exec_str,
                    "NOP (no memory access)",
                    "PC updated (no register write)",
                );
            }
            0x6F => {
                // jal
                if rd > 0 {
                    undo_log.push(UndoEntry::RegWrite {
                        reg: rd,
                        old_value: regs[rd],
                        new_value: next_pc,
                    });
                    regs[rd] = next_pc;
                }
                let target = pc.wrapping_add(imm_j as u32);
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: target,
                });
                events.push(TraceEvent::RegWrite);
                let wb_str = if rd > 0 {
                    format!("x{} ← PC+4 (0x{:08X}), PC ← 0x{:08X}", rd, next_pc, target)
                } else {
                    format!("PC ← 0x{:08X} (rd=x0)", target)
                };
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rd=x{}, imm (J-type)", rd),
                    &format!("Execute: target = PC + {} = 0x{:08X}", imm_j, target),
                    "NOP (no memory access)",
                    &wb_str,
                );
            }
            0x67 => {
                // jalr
                let target = (regs[rs1].wrapping_add(imm_i)) & !1;
                if rd > 0 {
                    undo_log.push(UndoEntry::RegWrite {
                        reg: rd,
                        old_value: regs[rd],
                        new_value: next_pc,
                    });
                    regs[rd] = next_pc;
                }
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: target,
                });
                events.push(TraceEvent::RegWrite);
                let wb_str = if rd > 0 {
                    format!("x{} ← PC+4, PC ← (x{} + {}) & !1 = 0x{:08X}", rd, rs1, imm_i as i32, target)
                } else {
                    format!("PC ← (x{} + {}) & !1 = 0x{:08X}", rs1, imm_i as i32, target)
                };
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    &format!("Extract rd=x{}, rs1=x{}, imm={}", rd, rs1, imm_i as i32),
                    &format!("Execute: target = (x{} + {}) & !1 = 0x{:08X}", rs1, imm_i as i32, target),
                    "NOP (no memory access)",
                    &wb_str,
                );
            }
            0x73 => {
                // ecall / ebreak: imm[11:0]=1 -> ebreak (halt)
                let imm_12 = (instr >> 20) & 0xFFF;
                if imm_12 == 1 {
                    // ebreak: halt for debugger
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    return StepResult {
                        new_state: CpuState { pc: next_pc, regs, halted: true },
                        events: vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Halted],
                        undo_log,
                        cycles_added: 3,
                        halted: true,
                        error: None,
                        instruction_bits: Some(instr),
                        pipeline_stages: pipeline_halt(
                            Some(instr),
                            &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                            "Decode: ebreak (breakpoint)",
                            "Halted",
                        ),
                        io_output: None,
                        io_input_requested: None,
                    };
                }
                // ecall - a7=10 exit, a7=11 print_int, a7=12 print_char, a7=5 read_int, a7=8 read_string, a7=12 read_char
                let a7 = regs[17];
                let a0 = regs[10] as i32;
                let a1 = regs[11];
                let (halt, io_out) = match a7 {
                    10 => (true, None),  // exit
                    93 => (true, None),  // exit with code (a0); we just halt
                    4 => {
                        // Print string: a0 = address of null-terminated string
                        let mut s = String::new();
                        let mut addr = regs[10];
                        loop {
                            match mem.read_u8(addr) {
                                Ok(0) => break,
                                Ok(b) => s.push(b as char),
                                Err(_) => break,
                            }
                            addr = addr.wrapping_add(1);
                        }
                        (false, Some(s))
                    }
                    11 => (false, Some(format!("{}", a0))),
                    12 => (false, Some((regs[10] as u8 as char).to_string())),
                    5 => {
                        match input {
                            Some(s) => {
                                let val = s.trim().parse::<i32>().unwrap_or(0);
                                if regs.len() > 10 {
                                    undo_log.push(UndoEntry::RegWrite { reg: 10, old_value: regs[10], new_value: val as u32 });
                                    regs[10] = val as u32;
                                    events.push(TraceEvent::RegWrite);
                                }
                                (false, None)
                            }
                            None => {
                                return StepResult {
                                    new_state: state.clone(),
                                    events: vec![TraceEvent::Fetch, TraceEvent::Decode],
                                    undo_log: vec![],
                                    cycles_added: 0,
                                    halted: false,
                                    error: None,
                                    instruction_bits: Some(instr),
                                    pipeline_stages: vec![
                                        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: Some(instr), action: format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc) },
                                        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: Some(instr), action: "ecall 5 (read int) – waiting for input".into() },
                                    ],
                                    io_output: None,
                                    io_input_requested: Some(InputRequest { kind: "int".into(), prompt: "Enter an integer".into(), max_length: None }),
                                };
                            }
                        }
                    }
                    8 => {
                        match input {
                            Some(s) => {
                                events.push(TraceEvent::Mem);
                                let buf_addr = regs[10];
                                let max_len = (a1 as usize).min(1024).max(1);
                                let bytes = s.as_bytes();
                                let to_write = bytes.len().min(max_len - 1);
                                for (i, &b) in bytes.iter().take(to_write).enumerate() {
                                    if let Ok(old) = mem.write_u8(buf_addr + i as u32, b) {
                                        undo_log.push(UndoEntry::MemWrite { addr: buf_addr + i as u32, old_value: old, new_value: b });
                                    }
                                }
                                if let Ok(old) = mem.write_u8(buf_addr + to_write as u32, 0) {
                                    undo_log.push(UndoEntry::MemWrite { addr: buf_addr + to_write as u32, old_value: old, new_value: 0 });
                                }
                                (false, None)
                            }
                            None => {
                                return StepResult {
                                    new_state: state.clone(),
                                    events: vec![TraceEvent::Fetch, TraceEvent::Decode],
                                    undo_log: vec![],
                                    cycles_added: 0,
                                    halted: false,
                                    error: None,
                                    instruction_bits: Some(instr),
                                    pipeline_stages: vec![
                                        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: Some(instr), action: format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc) },
                                        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: Some(instr), action: "ecall 8 (read string) – waiting for input".into() },
                                    ],
                                    io_output: None,
                                    io_input_requested: Some(InputRequest { kind: "string".into(), prompt: "Enter a string".into(), max_length: Some(a1) }),
                                };
                            }
                        }
                    }
                    13 => {
                        // Read char into a0 (use 13 to avoid conflict with 12=print char)
                        match input {
                            Some(s) => {
                                let c = s.chars().next().unwrap_or('\0');
                                if regs.len() > 10 {
                                    undo_log.push(UndoEntry::RegWrite { reg: 10, old_value: regs[10], new_value: c as u32 & 0xFF });
                                    regs[10] = c as u32 & 0xFF;
                                    events.push(TraceEvent::RegWrite);
                                }
                                (false, None)
                            }
                            None => {
                                return StepResult {
                                    new_state: state.clone(),
                                    events: vec![TraceEvent::Fetch, TraceEvent::Decode],
                                    undo_log: vec![],
                                    cycles_added: 0,
                                    halted: false,
                                    error: None,
                                    instruction_bits: Some(instr),
                                    pipeline_stages: vec![
                                        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: Some(instr), action: format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc) },
                                        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: Some(instr), action: "ecall 13 (read char) – waiting for input".into() },
                                    ],
                                    io_output: None,
                                    io_input_requested: Some(InputRequest { kind: "char".into(), prompt: "Enter a character".into(), max_length: Some(1) }),
                                };
                            }
                        }
                    }
                    _ if imm_i == 0 => (true, None),
                    _ => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(format!("Unsupported ecall a7={}", a7)),
                            instruction_bits: Some(instr),
                            pipeline_stages: pipeline_empty(),
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next_pc,
                });
                let events_out = if halt {
                    vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Halted]
                } else {
                    vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Alu, TraceEvent::RegWrite]
                };
                let action_str = if let Some(ref s) = io_out {
                    format!("ecall: print \"{}\"", s.replace('\n', "\\n"))
                } else if a7 == 93 {
                    format!("ecall 93: exit with code {}", a0)
                } else {
                    "ecall: halt".to_string()
                };
                return StepResult {
                    new_state: CpuState {
                        pc: next_pc,
                        regs,
                        halted: halt,
                    },
                    events: events_out,
                    undo_log,
                    cycles_added: if halt { 3 } else { 5 },
                    halted: halt,
                    error: None,
                    instruction_bits: Some(instr),
                    pipeline_stages: if halt {
                        pipeline_halt(
                            Some(instr),
                            &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                            "Decode: ecall (system call)",
                            "Halted",
                        )
                    } else {
                        pipeline_5(
                            Some(instr),
                            &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                            "Decode: ecall",
                            &action_str,
                            "NOP",
                            "PC ← PC+4",
                        )
                    },
                    io_output: io_out,
                    io_input_requested: None,
                };
            }
            0x00 => {
                // 0x00000000 - treat as NOP (padding/uninitialized memory)
                undo_log.push(UndoEntry::Pc {
                    old_value: pc,
                    new_value: next_pc,
                });
                pipeline_stages = pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                    "Decode: NOP (all zeros)",
                    "Execute: NOP",
                    "NOP (no memory access)",
                    "PC ← PC+4 (no register write)",
                );
            }
            _ => {
                let err_msg = format!(
                    "Runtime error at PC=0x{:08X}: Unknown opcode 0x{:02X}. \
                    Instruction word: 0x{:08X}. \
                    Possible causes: invalid/unsupported instruction, wrong jump target (e.g. jal/j label), or corrupted memory.",
                    pc, opcode, instr
                );
                return StepResult {
                    new_state: state.clone(),
                    events: vec![],
                    undo_log: vec![],
                    cycles_added: 0,
                    halted: false,
                    error: Some(err_msg),
                    instruction_bits: Some(instr),
                    pipeline_stages: pipeline_empty(),
                    io_output: None,
                io_input_requested: None,
                };
            }
        }

        StepResult {
            new_state: CpuState {
                pc: undo_log
                    .iter()
                    .rev()
                    .find_map(|e| {
                        if let UndoEntry::Pc { new_value, .. } = e {
                            Some(*new_value)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(next_pc),
                regs,
                halted: false,
            },
            events,
            undo_log,
            cycles_added: 5,
            halted: false,
            error: None,
            instruction_bits: Some(instr),
            pipeline_stages,
            io_output: None,
                io_input_requested: None,
        }
    }

    fn ui_schema(&self) -> UiSchema {
        // Inner CPU view: PC -> IM -> IR -> (RegFile, ALU, DM) -> MUX -> RegFile, Control
        UiSchema {
            blocks: vec![
                UiBlock {
                    id: "pc".into(),
                    label: "PC".into(),
                    x: 10.0,
                    y: 10.0,
                    width: 70.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "im".into(),
                    label: "Instr Mem".into(),
                    x: 95.0,
                    y: 10.0,
                    width: 75.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "ir".into(),
                    label: "IR".into(),
                    x: 185.0,
                    y: 10.0,
                    width: 60.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "regfile".into(),
                    label: "Registers".into(),
                    x: 185.0,
                    y: 70.0,
                    width: 75.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "alu".into(),
                    label: "ALU".into(),
                    x: 280.0,
                    y: 45.0,
                    width: 70.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "dm".into(),
                    label: "Data Mem".into(),
                    x: 280.0,
                    y: 105.0,
                    width: 75.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "mux".into(),
                    label: "MUX".into(),
                    x: 370.0,
                    y: 70.0,
                    width: 55.0,
                    height: 45.0,
                },
                UiBlock {
                    id: "control".into(),
                    label: "Control".into(),
                    x: 10.0,
                    y: 100.0,
                    width: 115.0,
                    height: 55.0,
                },
            ],
            connections: vec![
                UiConnection { from: "pc".into(), to: "im".into() },
                UiConnection { from: "im".into(), to: "ir".into() },
                UiConnection { from: "ir".into(), to: "regfile".into() },
                UiConnection { from: "ir".into(), to: "alu".into() },
                UiConnection { from: "regfile".into(), to: "alu".into() },
                UiConnection { from: "regfile".into(), to: "dm".into() },
                UiConnection { from: "alu".into(), to: "mux".into() },
                UiConnection { from: "dm".into(), to: "mux".into() },
                UiConnection { from: "mux".into(), to: "regfile".into() },
                UiConnection { from: "ir".into(), to: "control".into() },
                UiConnection { from: "control".into(), to: "pc".into() },
            ],
        }
    }

    fn register_schema(&self) -> RegisterSchema {
        let reg_names: Vec<String> = (0..32)
            .map(|i| {
                let abi = [
                    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1",
                    "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7", "s2", "s3",
                    "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
                    "t5", "t6",
                ];
                format!("x{} ({})", i, abi[i])
            })
            .collect();
        RegisterSchema {
            pc_name: "PC".to_string(),
            reg_names,
        }
    }
}

fn patch_branch_or_jal(insn: u32, rel_halfwords: i32) -> u32 {
    let opcode = insn & 0x7F;
    if opcode == 0x6F {
        // JAL: rel_halfwords is (target-pc)/2; encoded imm must decode to byte offset
        let byte_offset = rel_halfwords.wrapping_mul(2);
        let imm = (byte_offset as u32) & 0x1FFFFF;
        let imm20 = (imm >> 20) & 1;
        let imm10_1 = (imm >> 1) & 0x3FF;
        let imm11 = (imm >> 11) & 1;
        let imm19_12 = (imm >> 12) & 0xFF;
        (insn & 0xFFF) | (imm20 << 31) | (imm19_12 << 12) | (imm11 << 20) | (imm10_1 << 21)
    } else if opcode == 0x63 {
        // B-type: rel_halfwords is (target-pc)/2; encoded imm must decode to byte offset
        // RISC-V B-type immediate = byte offset (multiple of 2), so use rel_halfwords * 2
        let byte_offset = rel_halfwords.wrapping_mul(2);
        let imm = (byte_offset as u32) & 0x1FFF;
        let b12 = (imm >> 12) & 1;
        let b11 = (imm >> 11) & 1;
        let b10_5 = (imm >> 5) & 0x3F;
        let b4_1 = (imm >> 1) & 0xF;
        (insn & 0x1FFF) | (b12 << 31) | (b10_5 << 25) | (b11 << 7) | (b4_1 << 8)
    } else {
        insn
    }
}

fn parse_instruction(
    line: &str,
    line_num: u32,
    col: u32,
    pc: u32,
    _labels: &HashMap<String, u32>,
    bytes: &mut Vec<u8>,
    source_map: &mut Vec<SourceMapEntry>,
    _errors: &mut Vec<AssemblerError>,
    pending_refs: &mut Vec<(usize, String, u32)>,
) -> Result<(), AssemblerError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(());
    }

    let mnemonic = tokens[0].to_lowercase();
    let args: Vec<&str> = tokens.iter().skip(1).copied().collect();

    source_map.push(SourceMapEntry { pc, line: line_num, column: col });

    let mut encode = |insn: u32| {
        let offset = bytes.len();
        bytes.extend_from_slice(&insn.to_le_bytes());
        offset
    };

    match mnemonic.as_str() {
        "addi" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "addi rd, rs1, imm".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 0, rs1, imm));
        }
        "add" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "add rd, rs1, rs2".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 0, rs1, rs2, 0));
        }
        "sub" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sub rd, rs1, rs2".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 0, rs1, rs2, 0x20));
        }
        "slt" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "slt rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 2, rs1, rs2, 0));
        }
        "sltu" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "sltu rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 3, rs1, rs2, 0));
        }
        "xor" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "xor rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 4, rs1, rs2, 0));
        }
        "or" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "or rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 6, rs1, rs2, 0));
        }
        "and" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "and rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 7, rs1, rs2, 0));
        }
        "sll" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "sll rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 1, rs1, rs2, 0));
        }
        "srl" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "srl rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 5, rs1, rs2, 0));
        }
        "sra" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "sra rd, rs1, rs2".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x33, rd, 5, rs1, rs2, 0x20));
        }
        "slti" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "slti rd, rs1, imm".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 2, rs1, imm));
        }
        "sltiu" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "sltiu rd, rs1, imm".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 3, rs1, imm));
        }
        "xori" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "xori rd, rs1, imm".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 4, rs1, imm));
        }
        "ori" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "ori rd, rs1, imm".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 6, rs1, imm));
        }
        "andi" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "andi rd, rs1, imm".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 7, rs1, imm));
        }
        "slli" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "slli rd, rs1, shamt".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let shamt: u32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid shamt: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 1, rs1, (shamt & 0x1F) as i32));
        }
        "srli" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "srli rd, rs1, shamt".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let shamt: u32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid shamt: {}", args[2]) })?;
            encode(encode_i(0x13, rd, 5, rs1, (shamt & 0x1F) as i32));
        }
        "srai" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "srai rd, rs1, shamt".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let shamt: u32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid shamt: {}", args[2]) })?;
            encode((0x20u32 << 25) | ((shamt & 0x1F) << 20) | (rs1 << 15) | (5 << 12) | (rd << 7) | 0x13);
        }
        "lui" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "lui rd, imm".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let imm: u32 = if args[1].starts_with("0x") {
                u32::from_str_radix(args[1].trim_start_matches("0x"), 16).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?
            } else {
                args[1].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?
            };
            encode(encode_u(0x37, rd, imm));
        }
        "auipc" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "auipc rd, imm".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let imm: u32 = if args[1].starts_with("0x") {
                u32::from_str_radix(args[1].trim_start_matches("0x"), 16).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?
            } else {
                args[1].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?
            };
            encode(encode_u(0x17, rd, imm & 0xFFFFF000));
        }
        "ebreak" => {
            if !args.is_empty() {
                return Err(AssemblerError { line: line_num, column: col, message: "ebreak (no args)".to_string() });
            }
            encode(encode_i(0x73, 0, 0, 0, 1));
        }
        "lb" => {
            if args.len() != 2 { return Err(AssemblerError { line: line_num, column: col, message: "lb rd, offset(rs1)".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x03, rd, 0, rs1, imm));
        }
        "lh" => {
            if args.len() != 2 { return Err(AssemblerError { line: line_num, column: col, message: "lh rd, offset(rs1)".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x03, rd, 1, rs1, imm));
        }
        "lw" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "lw rd, offset(rs1)".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x03, rd, 2, rs1, imm));
        }
        "lbu" => {
            if args.len() != 2 { return Err(AssemblerError { line: line_num, column: col, message: "lbu rd, offset(rs1)".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x03, rd, 4, rs1, imm));
        }
        "lhu" => {
            if args.len() != 2 { return Err(AssemblerError { line: line_num, column: col, message: "lhu rd, offset(rs1)".to_string() }); }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x03, rd, 5, rs1, imm));
        }
        "sw" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "sw rs2, offset(rs1)".to_string() });
            }
            let rs2 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_s(0x23, imm, 2, rs1, rs2));
        }
        "sb" => {
            if args.len() != 2 { return Err(AssemblerError { line: line_num, column: col, message: "sb rs2, offset(rs1)".to_string() }); }
            let rs2 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_s(0x23, imm, 0, rs1, rs2));
        }
        "sh" => {
            if args.len() != 2 { return Err(AssemblerError { line: line_num, column: col, message: "sh rs2, offset(rs1)".to_string() }); }
            let rs2 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_s(0x23, imm, 1, rs1, rs2));
        }
        "beq" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "beq rs1, rs2, label".to_string() });
            }
            let rs1 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs2 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let (insn, is_label) = if let Ok(imm) = args[2].parse::<i32>() {
                (encode_b(0x63, imm, 0, rs1, rs2), false)
            } else {
                (encode_b(0x63, 0, 0, rs1, rs2), true)
            };
            let pos = encode(insn);
            if is_label {
                pending_refs.push((pos, args[2].to_string(), line_num));
            }
        }
        "bne" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "bne rs1, rs2, label".to_string() });
            }
            let rs1 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs2 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let (insn, is_label) = if let Ok(imm) = args[2].parse::<i32>() {
                (encode_b(0x63, imm, 1, rs1, rs2), false)
            } else {
                (encode_b(0x63, 0, 1, rs1, rs2), true)
            };
            let pos = encode(insn);
            if is_label {
                pending_refs.push((pos, args[2].to_string(), line_num));
            }
        }
        "blt" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "blt rs1, rs2, label".to_string() }); }
            let rs1 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs2 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let (insn, is_label) = if let Ok(imm) = args[2].parse::<i32>() { (encode_b(0x63, imm, 4, rs1, rs2), false) } else { (encode_b(0x63, 0, 4, rs1, rs2), true) };
            let pos = encode(insn);
            if is_label { pending_refs.push((pos, args[2].to_string(), line_num)); }
        }
        "bge" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "bge rs1, rs2, label".to_string() }); }
            let rs1 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs2 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let (insn, is_label) = if let Ok(imm) = args[2].parse::<i32>() { (encode_b(0x63, imm, 5, rs1, rs2), false) } else { (encode_b(0x63, 0, 5, rs1, rs2), true) };
            let pos = encode(insn);
            if is_label { pending_refs.push((pos, args[2].to_string(), line_num)); }
        }
        "bltu" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "bltu rs1, rs2, label".to_string() }); }
            let rs1 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs2 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let (insn, is_label) = if let Ok(imm) = args[2].parse::<i32>() { (encode_b(0x63, imm, 6, rs1, rs2), false) } else { (encode_b(0x63, 0, 6, rs1, rs2), true) };
            let pos = encode(insn);
            if is_label { pending_refs.push((pos, args[2].to_string(), line_num)); }
        }
        "bgeu" => {
            if args.len() != 3 { return Err(AssemblerError { line: line_num, column: col, message: "bgeu rs1, rs2, label".to_string() }); }
            let rs1 = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs2 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let (insn, is_label) = if let Ok(imm) = args[2].parse::<i32>() { (encode_b(0x63, imm, 7, rs1, rs2), false) } else { (encode_b(0x63, 0, 7, rs1, rs2), true) };
            let pos = encode(insn);
            if is_label { pending_refs.push((pos, args[2].to_string(), line_num)); }
        }
        "j" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "j label".to_string() });
            }
            let (insn, is_label) = if let Ok(imm) = args[0].parse::<i32>() {
                (encode_j(0x6f, 0, imm), false)
            } else {
                (encode_j(0x6f, 0, 0), true)
            };
            let pos = encode(insn);
            if is_label {
                pending_refs.push((pos, args[0].to_string(), line_num));
            }
        }
        "jal" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "jal rd, label".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (insn, is_label) = if let Ok(imm) = args[1].parse::<i32>() {
                (encode_j(0x6F, rd, imm), false)
            } else {
                (encode_j(0x6F, rd, 0), true)
            };
            let pos = encode(insn);
            if is_label {
                pending_refs.push((pos, args[1].to_string(), line_num));
            }
        }
        "jalr" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "jalr rd, offset(rs1)".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (imm, rs1) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x67, rd, 0, rs1, imm));
        }
        "ecall" => {
            encode(encode_i(0x73, 0, 0, 0, 0));
        }
        "nop" => {
            encode(encode_i(0x13, 0, 0, 0, 0));
        }
        "li" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "li rd, imm".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let imm: i32 = if args[1].starts_with("0x") {
                let v = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?;
                v as i32
            } else {
                args[1].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?
            };
            encode(encode_i(0x13, rd, 0, 0, imm));
        }
        "mv" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "mv rd, rs1".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            encode(encode_i(0x13, rd, 0, rs1, 0));
        }
        "ret" => {
            if !args.is_empty() {
                return Err(AssemblerError { line: line_num, column: col, message: "ret (no args)".to_string() });
            }
            encode(encode_i(0x67, 0, 0, 1, 0));
        }
        _ => {
            return Err(AssemblerError { line: line_num, column: col, message: format!("Unknown instruction: {}", mnemonic) });
        }
    }

    Ok(())
}

fn parse_offset_base(s: &str) -> Result<(i32, u32), String> {
    // offset(rs1) or (rs1)
    let s = s.trim();
    if let Some(p) = s.find('(') {
        let offset_str = s[..p].trim();
        let base = s[p + 1..].trim().trim_end_matches(')');
        let imm: i32 = if offset_str.is_empty() {
            0
        } else {
            offset_str.parse().map_err(|_| format!("Invalid offset: {}", offset_str))?
        };
        let rs1 = parse_reg(base).ok_or_else(|| format!("Invalid reg: {}", base))?;
        Ok((imm, rs1))
    } else {
        Err(format!("Expected offset(rs1) format: {}", s))
    }
}
