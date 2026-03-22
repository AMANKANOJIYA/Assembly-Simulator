//! LC-3 (Little Computer 3) assembler and executor.
//! 16-bit instructions, 8 registers (R0-R7), 16-bit address space.
//! Supports: ADD, AND, NOT, BR, JMP, JSR, JSRR, LD, LDI, LDR, LEA, ST, STI, STR, TRAP, NOP

use crate::memory::Memory;
use crate::plugin::*;
use std::collections::HashMap;

pub struct Lc3Plugin;

impl Lc3Plugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Lc3Plugin {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_reg(name: &str) -> Option<u32> {
    let name = name.trim().trim_end_matches(',').to_lowercase();
    if name.starts_with('r') {
        let n = name[1..].parse::<u32>().ok()?;
        if n < 8 {
            return Some(n);
        }
    }
    None
}

fn sign_extend_5(x: u16) -> u16 {
    let x = x & 0x1F;
    if (x & 0x10) != 0 {
        x | 0xFFE0
    } else {
        x
    }
}

fn sign_extend_6(x: u16) -> u16 {
    let x = x & 0x3F;
    if (x & 0x20) != 0 {
        x | 0xFFC0
    } else {
        x
    }
}

fn sign_extend_9(x: u16) -> u16 {
    let x = x & 0x1FF;
    if (x & 0x100) != 0 {
        x | 0xFE00
    } else {
        x
    }
}

fn sign_extend_11(x: u16) -> u16 {
    let x = x & 0x7FF;
    if (x & 0x400) != 0 {
        x | 0xF800
    } else {
        x
    }
}

/// 5-stage pipeline helper
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

impl ArchitecturePlugin for Lc3Plugin {
    fn name(&self) -> &str {
        "LC3"
    }

    fn assemble(&self, source: &str) -> ProgramImage {
        let mut bytes = Vec::new();
        let mut source_map = Vec::new();
        let mut errors = Vec::new();
        let mut labels: HashMap<String, u32> = HashMap::new();
        // (byte_offset_in_output, pc_at_instruction, label, line_num, ref_type_bits)
        // For PC-relative encodings (BR/LD/LEA/...): immediate is in *words* relative to (PC+2).
        let mut pending_refs: Vec<(usize, u32, String, u32, u8)> = Vec::new();
        let mut start_pc: u32 = 0x3000;

        let lines: Vec<&str> = source.lines().collect();
        let mut pc: u32 = 0x3000;

        for (line_no, line) in lines.iter().enumerate() {
            let line_num = (line_no + 1) as u32;
            // LC-3 uses ; for comments. Do NOT split on # — it's the immediate prefix (e.g. #10)
            let line = line
                .split(';')
                .next()
                .unwrap_or(line)
                .trim();
            if line.is_empty() {
                continue;
            }

            let col = (line.find(|c: char| !c.is_whitespace()).unwrap_or(0) + 1) as u32;

            if line.to_uppercase().starts_with(".ORIG") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parse_lc3_hex_or_dec(parts[1]) {
                        start_pc = val & 0xFFFF;
                        pc = start_pc;
                    }
                }
                continue;
            }

            if line.to_uppercase().trim() == ".END" {
                break;
            }

            if let Some(idx) = line.find(':') {
                let label = line[..idx].trim().to_string();
                if !label.is_empty() && !label.starts_with('.') {
                    labels.insert(label, pc);
                }
                let rest = line[idx + 1..].trim();
                if rest.is_empty() {
                    continue;
                }
                match parse_lc3_instruction(
                    rest, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                    &mut errors, &mut pending_refs,
                ) {
                    Ok(words) => pc += 2 * words,
                    Err(e) => errors.push(e),
                }
                continue;
            }

            match parse_lc3_instruction(
                line, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                &mut errors, &mut pending_refs,
            ) {
                Ok(words) => pc += 2 * words,
                Err(e) => errors.push(e),
            }
        }

        for (byte_offset, pc_at_insn, label, line_num, ref_type) in pending_refs {
            if let Some(&target) = labels.get(&label) {
                // LC-3 PC-relative offsets are measured in *words* from the incremented PC (PC+2 bytes).
                let base = (pc_at_insn.wrapping_add(2)) & 0xFFFF;
                let diff_bytes = (target as i32) - (base as i32);
                let diff_words = diff_bytes / 2;

                let insn = u16::from_le_bytes([bytes[byte_offset], bytes[byte_offset + 1]]);
                let patched = match ref_type {
                    9 => {
                        if diff_words < -256 || diff_words > 255 {
                            errors.push(AssemblerError {
                                line: line_num,
                                column: 1,
                                message: format!("Label '{}' out of range for PCoffset9", label),
                            });
                            insn
                        } else {
                            (insn & 0xFE00) | ((diff_words as u16) & 0x01FF)
                        }
                    }
                    11 => {
                        if diff_words < -1024 || diff_words > 1023 {
                            errors.push(AssemblerError {
                                line: line_num,
                                column: 1,
                                message: format!("Label '{}' out of range for PCoffset11", label),
                            });
                            insn
                        } else {
                            // keep bit 11 (JSR vs JSRR selector), patch the remaining 11 bits
                            (insn & 0xF800) | ((diff_words as u16) & 0x07FF)
                        }
                    }
                    _ => insn,
                };
                bytes[byte_offset..byte_offset + 2].copy_from_slice(&patched.to_le_bytes());
            } else {
                errors.push(AssemblerError {
                    line: line_num,
                    column: 1,
                    message: format!("Unknown label: {}", label),
                });
            }
        }

        ProgramImage {
            entry_pc: labels.get("_start").copied().unwrap_or(start_pc),
            bytes,
            source_map,
            errors,
        }
    }

    fn reset(&self, _config: &ResetConfig) -> CpuState {
        CpuState {
            pc: 0x3000,
            regs: vec![0u32; 9], // R0-R7 + R8 = PSR (N<<2|Z<<1|P) for BR
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
                pipeline_stages: vec![],
                io_output: None,
                io_input_requested: None,
            };
        }

        let mut mem = Memory::new(memory.len());
        mem.data_mut().copy_from_slice(memory);

        let pc = state.pc & 0xFFFF;
        let mut regs = state.regs.clone();

        let instr_u16 = match mem.read_u16_le(pc) {
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
                    pipeline_stages: vec![],
                    io_output: None,
                io_input_requested: None,
                };
            }
        };
        let instr = instr_u16 as u32;
        let mut undo_log = Vec::new();
        let mut events = vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Alu];

        let opcode = (instr_u16 >> 12) & 0xF;
        let dr = ((instr_u16 >> 9) & 0x7) as usize;
        let sr1 = ((instr_u16 >> 6) & 0x7) as usize;
        let sr2 = (instr_u16 & 0x7) as usize;
        let imm5 = sign_extend_5((instr_u16 >> 5) & 0x1F);
        let offset6 = sign_extend_6(instr_u16 & 0x3F);
        let pcoffset9 = sign_extend_9(instr_u16 & 0x1FF);
        let pcoffset11 = sign_extend_11(instr_u16 & 0x7FF);
        let n = (instr_u16 >> 11) & 1;
        let z = (instr_u16 >> 10) & 1;
        let p = (instr_u16 >> 9) & 1;
        let base_r = ((instr_u16 >> 6) & 0x7) as usize;
        let trapvect = (instr_u16 & 0xFF) as u8;

        let next_pc = (pc + 2) & 0xFFFF;

fn set_cc(val: u16) -> (u32, u32, u32) {
    let v = val as i16;
    let n = if v < 0 { 1u32 } else { 0 };
    let z = if v == 0 { 1u32 } else { 0 };
    let p = if v > 0 { 1u32 } else { 0 };
    (n, z, p)
}

        let psr = regs[8]; // N<<2 | Z<<1 | P (hidden PSR)
        let cc_n = (psr >> 2) & 1;
        let cc_z = (psr >> 1) & 1;
        let cc_p = psr & 1;

        let pipeline_stages: Vec<PipelineCycleInfo> = match opcode {
            0b0001 => {
                // ADD: DR = SR1 + SR2 or DR = SR1 + imm5
                let imm_mode = (instr_u16 >> 5) & 1;
                let a = regs[sr1] as u16;
                let b = if imm_mode != 0 {
                    imm5
                } else {
                    regs[sr2] as u16
                };
                let result = (a as i16).wrapping_add(b as i16) as u16;
                let (n, z, p) = set_cc(result);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: result as u32 });
                regs[dr] = result as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    &format!("Extract DR=R{}, SR1=R{}", dr, sr1),
                    &format!("ALU: R{} + {} = 0x{:04X}", sr1, b, result),
                    "NOP",
                    &format!("R{} ← 0x{:04X}", dr, result),
                )
            }
            0b0101 => {
                // AND
                let imm_mode = (instr_u16 >> 5) & 1;
                let a = regs[sr1] as u16;
                let b = if imm_mode != 0 { imm5 } else { regs[sr2] as u16 };
                let result = a & b;
                let (n, z, p) = set_cc(result);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: result as u32 });
                regs[dr] = result as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode AND",
                    &format!("ALU: R{} & {} = 0x{:04X}", sr1, b, result),
                    "NOP",
                    &format!("R{} ← 0x{:04X}", dr, result),
                )
            }
            0b1001 => {
                // NOT: DR = ~SR1
                let a = regs[sr1] as u16;
                let result = !a & 0xFFFF;
                let (n, z, p) = set_cc(result);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: result as u32 });
                regs[dr] = result as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode NOT",
                    &format!("ALU: ~R{} = 0x{:04X}", sr1, result),
                    "NOP",
                    &format!("R{} ← 0x{:04X}", dr, result),
                )
            }
            0b0000 => {
                // BR
                let take = (n != 0 && cc_n != 0) || (z != 0 && cc_z != 0) || (p != 0 && cc_p != 0)
                    || (n == 0 && z == 0 && p == 0);
                // PC-relative offsets are in words; this simulator uses byte addressing, so scale by 2.
                let target = (next_pc.wrapping_add(((pcoffset9 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let next = if take { target } else { next_pc };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                let exec_str = if take {
                    format!("BR: taken → PC ← 0x{:04X}", target)
                } else {
                    "BR: not taken".to_string()
                };
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode BR",
                    &exec_str,
                    "NOP",
                    "PC updated",
                )
            }
            0b1100 => {
                // JMP / RET: PC = BaseR (already 16-bit)
                let target = (regs[base_r] & 0xFFFF) as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    &format!("Decode JMP R{}", base_r),
                    &format!("Execute: PC ← R{} = 0x{:04X}", base_r, target),
                    "NOP",
                    "PC updated",
                )
            }
            0b0100 => {
                if (instr_u16 & 0x0800) != 0 {
                    // JSR: R7 = PC+2, PC = PC + SEXT(PCoffset11)
                    let target = (next_pc.wrapping_add(((pcoffset11 as i16 as i32) * 2) as u32)) & 0xFFFF;
                    undo_log.push(UndoEntry::RegWrite { reg: 7, old_value: regs[7] as u32, new_value: next_pc as u32 });
                    regs[7] = next_pc;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                        "Decode JSR",
                        &format!("Execute: R7 ← PC+2, PC ← 0x{:04X}", target),
                        "NOP",
                        "R7, PC updated",
                    )
                } else {
                    // JSRR: R7 = PC+2, PC = BaseR
                    let target = (regs[base_r] & 0xFFFF) as u32;
                    undo_log.push(UndoEntry::RegWrite { reg: 7, old_value: regs[7] as u32, new_value: next_pc as u32 });
                    regs[7] = next_pc;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                        &format!("Decode JSRR R{}", base_r),
                        &format!("Execute: R7 ← PC+2, PC ← R{}", base_r),
                        "NOP",
                        "R7, PC updated",
                    )
                }
            }
            0b0010 => {
                // LD: DR = mem[PC + PCoffset9]
                events.push(TraceEvent::Mem);
                let addr = (next_pc.wrapping_add(((pcoffset9 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let val = match mem.read_u16_le(addr) {
                    Ok(v) => v,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                let (n, z, p) = set_cc(val);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: val as u32 });
                regs[dr] = val as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode LD",
                    &format!("addr = PC + {} = 0x{:04X}", pcoffset9 as i16, addr),
                    &format!("Mem[0x{:04X}] = 0x{:04X}", addr, val),
                    &format!("R{} ← 0x{:04X}", dr, val),
                )
            }
            0b1010 => {
                // LDI: DR = mem[mem[PC+PCoffset9]]
                events.push(TraceEvent::Mem);
                let ptr_addr = (next_pc.wrapping_add(((pcoffset9 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let addr = match mem.read_u16_le(ptr_addr) {
                    Ok(a) => (a as u32) & 0xFFFF,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                let val = match mem.read_u16_le(addr) {
                    Ok(v) => v,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                let (n, z, p) = set_cc(val);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: val as u32 });
                regs[dr] = val as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode LDI",
                    &format!("ptr = Mem[PC+offset] = 0x{:04X}", addr),
                    &format!("Mem[0x{:04X}] = 0x{:04X}", addr, val),
                    &format!("R{} ← 0x{:04X}", dr, val),
                )
            }
            0b0110 => {
                // LDR: DR = mem[BaseR + offset6]
                events.push(TraceEvent::Mem);
                let addr =
                    ((regs[base_r] as u32).wrapping_add(((offset6 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let val = match mem.read_u16_le(addr) {
                    Ok(v) => v,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                let (n, z, p) = set_cc(val);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: val as u32 });
                regs[dr] = val as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    &format!("Decode LDR R{}, R{}, {}", dr, base_r, offset6 as i16),
                    &format!("addr = R{} + {} = 0x{:04X}", base_r, offset6 as i16, addr),
                    &format!("Mem[0x{:04X}] = 0x{:04X}", addr, val),
                    &format!("R{} ← 0x{:04X}", dr, val),
                )
            }
            0b1110 => {
                // LEA: DR = PC + PCoffset9
                let val = next_pc.wrapping_add(((pcoffset9 as i16 as i32) * 2) as u32) & 0xFFFF;
                let (n, z, p) = set_cc(val as u16);
                regs[8] = (n << 2) | (z << 1) | p;
                undo_log.push(UndoEntry::RegWrite { reg: 8, old_value: psr, new_value: regs[8] });
                undo_log.push(UndoEntry::RegWrite { reg: dr, old_value: regs[dr] as u32, new_value: val as u32 });
                regs[dr] = val;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                events.push(TraceEvent::RegWrite);
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode LEA",
                    &format!("Execute: PC + {} = 0x{:04X}", pcoffset9 as i16, val),
                    "NOP",
                    &format!("R{} ← 0x{:04X}", dr, val),
                )
            }
            0b0011 => {
                // ST: mem[PC + PCoffset9] = SR
                events.push(TraceEvent::Mem);
                let addr = (next_pc.wrapping_add(((pcoffset9 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let val = regs[dr] as u16;
                let old = match mem.write_u16_le(addr, val) {
                    Ok(o) => o,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                for (i, &b) in old.iter().enumerate() {
                    undo_log.push(UndoEntry::MemWrite {
                        addr: addr + i as u32,
                        old_value: b,
                        new_value: (val >> (i * 8)) as u8,
                    });
                }
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    &format!("Decode ST R{}, offset", dr),
                    &format!("addr = PC + {} = 0x{:04X}", pcoffset9 as i16, addr),
                    &format!("Mem[0x{:04X}] ← 0x{:04X}", addr, val),
                    "NOP",
                )
            }
            0b1011 => {
                // STI: mem[mem[PC+PCoffset9]] = SR
                events.push(TraceEvent::Mem);
                let ptr_addr = (next_pc.wrapping_add(((pcoffset9 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let addr = match mem.read_u16_le(ptr_addr) {
                    Ok(a) => (a as u32) & 0xFFFF,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                let val = regs[dr] as u16;
                let old = match mem.write_u16_le(addr, val) {
                    Ok(o) => o,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                for (i, &b) in old.iter().enumerate() {
                    undo_log.push(UndoEntry::MemWrite { addr: addr + i as u32, old_value: b, new_value: (val >> (i * 8)) as u8 });
                }
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    "Decode STI",
                    &format!("ptr = Mem[PC+offset] = 0x{:04X}", addr),
                    &format!("Mem[0x{:04X}] ← 0x{:04X}", addr, val),
                    "NOP",
                )
            }
            0b0111 => {
                // STR: mem[BaseR + offset6] = SR
                events.push(TraceEvent::Mem);
                let addr =
                    ((regs[base_r] as u32).wrapping_add(((offset6 as i16 as i32) * 2) as u32)) & 0xFFFF;
                let val = regs[dr] as u16;
                let old = match mem.write_u16_le(addr, val) {
                    Ok(o) => o,
                    Err(e) => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(e),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                for (i, &b) in old.iter().enumerate() {
                    undo_log.push(UndoEntry::MemWrite { addr: addr + i as u32, old_value: b, new_value: (val >> (i * 8)) as u8 });
                }
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                pipeline_5(
                    Some(instr),
                    &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                    &format!("Decode STR R{}, R{}, {}", dr, base_r, offset6 as i16),
                    &format!("addr = R{} + {} = 0x{:04X}", base_r, offset6 as i16, addr),
                    &format!("Mem[0x{:04X}] ← 0x{:04X}", addr, val),
                    "NOP",
                )
            }
            0b1111 => {
                // TRAP
                let (halt, io_out, reg_write) = match trapvect {
                    0x25 => (true, None, None), // HALT
                    0x20 => (false, Some(format!("{}", (regs[0] & 0xFF) as u8 as char)), None), // OUT
                    0x21 => {
                        // PUTS: R0 = address of null-terminated string
                        let mut s = String::new();
                        let mut addr = (regs[0] as u32) & 0xFFFF;
                        loop {
                            match mem.read_u8(addr) {
                                Ok(0) => break,
                                Ok(b) => s.push(b as char),
                                Err(_) => break,
                            }
                            addr += 1;
                        }
                        (false, Some(s), None)
                    }
                    0x24 => {
                        // PUTSP: R0 = address of string; each word = two bytes (low byte first), stop at 0x0000
                        let mut s = String::new();
                        let mut addr = (regs[0] as u32) & 0xFFFF;
                        loop {
                            let word = match mem.read_u16_le(addr) {
                                Ok(w) => w,
                                Err(_) => break,
                            };
                            if word == 0 {
                                break;
                            }
                            let lo = (word & 0xFF) as u8;
                            let hi = (word >> 8) as u8;
                            if lo != 0 {
                                s.push(lo as char);
                            }
                            if hi != 0 {
                                s.push(hi as char);
                            }
                            addr += 2;
                        }
                        (false, Some(s), None)
                    }
                    0x22 => {
                        // IN: read char, store in R0, echo to output
                        match input {
                            Some(s) => {
                                let c = s.chars().next().unwrap_or('\0');
                                (false, Some(c.to_string()), Some(c as u32 & 0xFF))
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
                                        PipelineCycleInfo {
                                            stage: "Fetch".into(),
                                            instruction_bits: Some(instr),
                                            action: format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                                        },
                                        PipelineCycleInfo {
                                            stage: "Decode".into(),
                                            instruction_bits: Some(instr),
                                            action: "Decode TRAP x22 (IN) – waiting for input".into(),
                                        },
                                    ],
                                    io_output: None,
                                    io_input_requested: Some(InputRequest {
                                        kind: "char".into(),
                                        prompt: "Enter a character".into(),
                                        max_length: Some(1),
                                    }),
                                };
                            }
                        }
                    }
                    0x23 => {
                        // GETC: read char, store in R0, no echo
                        match input {
                            Some(s) => {
                                let c = s.chars().next().unwrap_or('\0');
                                (false, None, Some(c as u32 & 0xFF))
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
                                        PipelineCycleInfo {
                                            stage: "Fetch".into(),
                                            instruction_bits: Some(instr),
                                            action: format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                                        },
                                        PipelineCycleInfo {
                                            stage: "Decode".into(),
                                            instruction_bits: Some(instr),
                                            action: "Decode TRAP x23 (GETC) – waiting for input".into(),
                                        },
                                    ],
                                    io_output: None,
                                    io_input_requested: Some(InputRequest {
                                        kind: "char".into(),
                                        prompt: "Enter a character (no echo)".into(),
                                        max_length: Some(1),
                                    }),
                                };
                            }
                        }
                    }
                    _ => {
                        return StepResult {
                            new_state: state.clone(),
                            events: vec![],
                            undo_log: vec![],
                            cycles_added: 0,
                            halted: false,
                            error: Some(format!("Unsupported TRAP 0x{:02X}", trapvect)),
                            instruction_bits: Some(instr),
                            pipeline_stages: vec![],
                            io_output: None,
                io_input_requested: None,
                        };
                    }
                };
                if let Some(val) = reg_write {
                    undo_log.push(UndoEntry::RegWrite { reg: 0, old_value: regs[0], new_value: val });
                    regs[0] = val;
                    events.push(TraceEvent::RegWrite);
                }
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                let action_str = match (trapvect, &io_out, reg_write) {
                    (0x21, Some(s), _) => format!("TRAP: PUTS \"{}\"", s.replace('\n', "\\n")),
                    (0x24, Some(s), _) => format!("TRAP: PUTSP \"{}\"", s.replace('\n', "\\n")),
                    (0x22, _, Some(v)) => format!("TRAP: IN → R0 ← '{}' (0x{:02X})", (v & 0xFF) as u8 as char, v & 0xFF),
                    (0x23, _, Some(v)) => format!("TRAP: GETC → R0 ← '{}' (0x{:02X})", (v & 0xFF) as u8 as char, v & 0xFF),
                    (0x25, _, _) => "TRAP x25: HALT".to_string(),
                    (_, Some(s), _) => format!("TRAP: OUT \"{}\"", s.replace('\n', "\\n")),
                    _ => "TRAP".to_string(),
                };
                return StepResult {
                    new_state: CpuState { pc: next_pc, regs, halted: halt },
                    events: if halt { vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Halted] } else { events },
                    undo_log,
                    cycles_added: if halt { 3 } else { 5 },
                    halted: halt,
                    error: None,
                    instruction_bits: Some(instr),
                    pipeline_stages: if halt {
                        pipeline_halt(
                            Some(instr),
                            &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                            "Decode: TRAP x25 HALT",
                            "Halted",
                        )
                    } else {
                        pipeline_5(
                            Some(instr),
                            &format!("Load 0x{:04X} from IMem[PC=0x{:04X}]", instr_u16, pc),
                            "Decode TRAP",
                            &action_str,
                            "NOP",
                            "PC ← PC+2",
                        )
                    },
                    io_output: io_out,
                    io_input_requested: None,
                };
            }
            _ => {
                return StepResult {
                    new_state: state.clone(),
                    events: vec![],
                    undo_log: vec![],
                    cycles_added: 0,
                    halted: false,
                    error: Some(format!("Unknown LC-3 opcode 0x{:X} at PC=0x{:04X}", opcode, pc)),
                    instruction_bits: Some(instr),
                    pipeline_stages: vec![],
                    io_output: None,
                io_input_requested: None,
                };
            }
        };

        let next = undo_log.iter().rev().find_map(|e| {
            if let UndoEntry::Pc { new_value, .. } = e {
                Some(*new_value)
            } else {
                None
            }
        }).unwrap_or(next_pc);

        StepResult {
            new_state: CpuState { pc: next, regs, halted: false },
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
        UiSchema {
            blocks: vec![
                UiBlock { id: "pc".into(), label: "PC".into(), x: 10.0, y: 10.0, width: 70.0, height: 45.0 },
                UiBlock { id: "im".into(), label: "Instr Mem".into(), x: 95.0, y: 10.0, width: 75.0, height: 45.0 },
                UiBlock { id: "ir".into(), label: "IR".into(), x: 185.0, y: 10.0, width: 60.0, height: 45.0 },
                UiBlock { id: "regfile".into(), label: "R0-R7".into(), x: 185.0, y: 70.0, width: 75.0, height: 45.0 },
                UiBlock { id: "alu".into(), label: "ALU".into(), x: 280.0, y: 45.0, width: 70.0, height: 45.0 },
                UiBlock { id: "dm".into(), label: "Data Mem".into(), x: 280.0, y: 105.0, width: 75.0, height: 45.0 },
                UiBlock { id: "mux".into(), label: "MUX".into(), x: 370.0, y: 70.0, width: 55.0, height: 45.0 },
                UiBlock { id: "control".into(), label: "Control".into(), x: 10.0, y: 100.0, width: 115.0, height: 55.0 },
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
        RegisterSchema {
            pc_name: "PC".to_string(),
            reg_names: (0..8).map(|i| format!("R{}", i)).chain(std::iter::once("PSR (NZP)".to_string())).collect(),
        }
    }
}

/// Parse LC-3 immediate: #5, #-1, x2A, 10 (decimal)
fn parse_lc3_imm(s: &str) -> Result<i16, ()> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.to_lowercase().starts_with('x') || s.to_lowercase().starts_with("0x") {
        let hex = s.trim_start_matches('x').trim_start_matches('X').trim_start_matches("0x");
        i16::from_str_radix(hex, 16).map_err(|_| ())
    } else {
        s.parse::<i16>().map_err(|_| ())
    }
}

fn parse_lc3_hex_or_dec(s: &str) -> Result<u32, ()> {
    let s = s.trim();
    if s.to_lowercase().starts_with("0x") || s.to_lowercase().starts_with('x') {
        let hex = s.trim_start_matches('x').trim_start_matches('X').trim_start_matches("0x");
        u32::from_str_radix(hex, 16).map_err(|_| ())
    } else {
        s.parse::<u32>().map_err(|_| ())
    }
}

fn parse_lc3_instruction(
    line: &str,
    line_num: u32,
    col: u32,
    pc: u32,
    labels: &HashMap<String, u32>,
    bytes: &mut Vec<u8>,
    source_map: &mut Vec<SourceMapEntry>,
    errors: &mut Vec<AssemblerError>,
    pending_refs: &mut Vec<(usize, u32, String, u32, u8)>,
) -> Result<u32, AssemblerError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(0);
    }

    source_map.push(SourceMapEntry { pc, line: line_num, column: col });

    let mnemonic = tokens[0].to_uppercase();
    let args: Vec<&str> = tokens.iter().skip(1).copied().collect();

    let mut encode = |insn: u16| {
        let offset = bytes.len();
        bytes.extend_from_slice(&insn.to_le_bytes());
        offset
    };

    match mnemonic.as_str() {
        ".ORIG" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: ".ORIG x3000".to_string() });
            }
            let val: u32 = if args[0].starts_with("0x") || args[0].starts_with("x") {
                u32::from_str_radix(args[0].trim_start_matches('x').trim_start_matches("0x"), 16)
                    .map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid: {}", args[0]) })?
            } else {
                args[0].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid: {}", args[0]) })?
            };
            // .ORIG doesn't emit bytes; it sets start address. We use it as a label. Skip encoding.
            return Ok(0);
        }
        ".FILL" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: ".FILL value".to_string() });
            }
            let val: u16 = if args[0].to_lowercase().starts_with("0x") || args[0].to_lowercase().starts_with('x') {
                u16::from_str_radix(args[0].trim_start_matches('x').trim_start_matches('X').trim_start_matches("0x"), 16)
                    .map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid value: {}", args[0]) })?
            } else {
                args[0].parse::<i32>()
                    .map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid value: {}", args[0]) })?
                    as u16
            };
            bytes.extend_from_slice(&val.to_le_bytes());
            return Ok(1);
        }
        ".BLKW" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: ".BLKW count".to_string() });
            }
            let n: u32 = args[0].parse()
                .map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid count: {}", args[0]) })?;
            for _ in 0..n {
                bytes.extend_from_slice(&0u16.to_le_bytes());
            }
            return Ok(n);
        }
        "ADD" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "ADD DR, SR1, SR2 or ADD DR, SR1, #imm5".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let sr1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            if let Ok(imm) = parse_lc3_imm(args[2]) {
                let imm5 = (imm & 0x1F) as u16;
                encode((0x1000u32 | (dr << 9) | (sr1 << 6) | 0x20 | (imm5 as u32)) as u16);
            } else {
                let sr2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
                encode((0x1000u32 | (dr << 9) | (sr1 << 6) | (sr2 & 7)) as u16);
            }
        }
        "AND" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "AND DR, SR1, SR2 or AND DR, SR1, #imm5".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let sr1 = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            if let Ok(imm) = parse_lc3_imm(args[2]) {
                let imm5 = (imm & 0x1F) as u16;
                encode((0x5000u32 | (dr << 9) | (sr1 << 6) | 0x20 | (imm5 as u32)) as u16);
            } else {
                let sr2 = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
                encode((0x5000u32 | (dr << 9) | (sr1 << 6) | (sr2 & 7)) as u16);
            }
        }
        "NOT" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "NOT DR, SR".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let sr = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            encode((0x903Fu32 | (dr << 9) | (sr << 6)) as u16);
        }
        "BR" | "BRN" | "BRZ" | "BRP" | "BRNZ" | "BRNP" | "BRZP" | "BRNZP" => {
            let (n, z, p) = match mnemonic.as_str() {
                "BR" | "BRNZP" => (1, 1, 1),
                "BRN" => (1, 0, 0),
                "BRZ" => (0, 1, 0),
                "BRP" => (0, 0, 1),
                "BRNZ" => (1, 1, 0),
                "BRNP" => (1, 0, 1),
                "BRZP" => (0, 1, 1),
                _ => (1, 1, 1),
            };
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "BR label".to_string() });
            }
            let base: u16 = 0x0000 | (n << 11) | (z << 10) | (p << 9);
            let pos = encode(base);
            pending_refs.push((pos, pc, args[0].to_string(), line_num, 9));
        }
        "JMP" | "RET" => {
            if mnemonic == "RET" {
                encode(0xC1C0u16);
            } else if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "JMP BaseR".to_string() });
            } else {
                let base = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
                encode((0xC000u32 | (base << 6)) as u16);
            }
        }
        "JSR" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "JSR label".to_string() });
            }
            let pos = encode(0x4800u16);
            pending_refs.push((pos, pc, args[0].to_string(), line_num, 11));
        }
        "JSRR" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "JSRR BaseR".to_string() });
            }
            let base = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            encode((0x4000u32 | (base << 6)) as u16);
        }
        "LD" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "LD DR, label".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let pos = encode((0x2000u32 | (dr << 9)) as u16);
            pending_refs.push((pos, pc, args[1].to_string(), line_num, 9));
        }
        "LDI" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "LDI DR, label".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let pos = encode((0xA000u32 | (dr << 9)) as u16);
            pending_refs.push((pos, pc, args[1].to_string(), line_num, 9));
        }
        "LDR" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "LDR DR, BaseR, #offset6".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let base = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let offset: i16 = parse_lc3_imm(args[2]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid offset: {}", args[2]) })?;
            let off6 = (offset & 0x3F) as u16;
            encode((0x6000u32 | (dr << 9) | (base << 6) | (off6 as u32)) as u16);
        }
        "LEA" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "LEA DR, label".to_string() });
            }
            let dr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let pos = encode((0xE000u32 | (dr << 9)) as u16);
            pending_refs.push((pos, pc, args[1].to_string(), line_num, 9));
        }
        "ST" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "ST SR, label".to_string() });
            }
            let sr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let pos = encode((0x3000u32 | (sr << 9)) as u16);
            pending_refs.push((pos, pc, args[1].to_string(), line_num, 9));
        }
        "STI" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "STI SR, label".to_string() });
            }
            let sr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let pos = encode((0xB000u32 | (sr << 9)) as u16);
            pending_refs.push((pos, pc, args[1].to_string(), line_num, 9));
        }
        "STR" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "STR SR, BaseR, #offset6".to_string() });
            }
            let sr = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let base = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let offset: i16 = parse_lc3_imm(args[2]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid offset: {}", args[2]) })?;
            let off6 = (offset & 0x3F) as u16;
            encode((0x7000u32 | (sr << 9) | (base << 6) | (off6 as u32)) as u16);
        }
        "TRAP" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "TRAP trapvect8".to_string() });
            }
            let v: u16 = if args[0].starts_with("x") || args[0].starts_with("0x") {
                u16::from_str_radix(args[0].trim_start_matches('x').trim_start_matches("0x"), 16)
                    .map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid: {}", args[0]) })?
            } else {
                args[0].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid: {}", args[0]) })?
            };
            encode(0xF000 | (v & 0xFF));
        }
        "HALT" => { encode(0xF025u16); }
        "NOP" => { encode(0x0000u16); }
        ".END" => { return Ok(0); }
        _ => return Err(AssemblerError { line: line_num, column: col, message: format!("Unknown instruction: {} (labels need a colon, e.g. _start:)", mnemonic) }),
    }

    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lc3_assembles_pc_relative_labels_from_pc_plus_2_in_words() {
        let p = Lc3Plugin::new();
        // At x3000: BRnzp target; next_pc = x3002; target at x3006 => diff_bytes=4 => diff_words=2
        let src = r#"
.ORIG x3000
_start:
  BRnzp target
  NOP
  NOP
target:
  NOP
.END
"#;
        let img = p.assemble(src);
        assert!(img.errors.is_empty(), "errors: {:?}", img.errors);
        let br = u16::from_le_bytes([img.bytes[0], img.bytes[1]]);
        // BR with NZP=111 sets bits 11..9 => 0b111 at 0x0E00
        assert_eq!(br & 0xFE00, 0x0E00);
        assert_eq!(br & 0x01FF, 2);
    }

    #[test]
    fn lc3_executes_pc_relative_using_next_pc_and_word_scaling() {
        let p = Lc3Plugin::new();
        // BRnzp to the label two bytes ahead (1 word) should jump to x3004.
        let src = r#"
.ORIG x3000
_start:
  BRnzp target
  NOP
target:
  NOP
.END
"#;
        let img = p.assemble(src);
        assert!(img.errors.is_empty(), "errors: {:?}", img.errors);
        let mut mem = vec![0u8; 0x10000];
        mem[0x3000..0x3000 + img.bytes.len()].copy_from_slice(&img.bytes);
        let state = CpuState { pc: 0x3000, regs: vec![0u32; 9], halted: false };
        let r = p.step(&state, &mem, StepMode::Instruction, None);
        assert!(r.error.is_none(), "error: {:?}", r.error);
        assert_eq!(r.new_state.pc, 0x3004);
    }
}
