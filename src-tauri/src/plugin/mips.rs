//! MIPS32 full base assembler and executor.
//! Supports: add, sub, and, or, nor, xor, sll, srl, sra, sllv, srlv, srav, slt, sltu, slti, sltiu,
//! addi, xori, lb, lh, lw, sb, sh, sw, beq, bne, j, jal, jr, jalr, syscall, mult, multu, div, divu, mfhi, mflo, li, lui, nop.
//! HI/LO in regs[32], regs[33].

use crate::memory::Memory;
use crate::plugin::*;
use std::collections::HashMap;

pub struct MipsPlugin;

impl MipsPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MipsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_reg(name: &str) -> Option<u32> {
    let name = name.trim().trim_end_matches(',').to_lowercase();
    if name.starts_with('$') {
        let rest = &name[1..];
        if rest == "zero" { return Some(0); }
        if rest == "at" { return Some(1); }
        if rest == "v0" { return Some(2); }
        if rest == "v1" { return Some(3); }
        if rest.starts_with("a") {
            let n: u32 = rest[1..].parse().ok()?;
            if n <= 3 { return Some(4 + n); }
        }
        if rest.starts_with("t") {
            let n: u32 = rest[1..].parse().ok()?;
            if n <= 7 { return Some(8 + n); }
            if n <= 9 { return Some(24 + (n - 8)); }
        }
        // Check special regs (gp, sp, fp, ra) before generic s0-s7 — "sp" would wrongly match starts_with("s")
        if rest == "k0" { return Some(26); }
        if rest == "k1" { return Some(27); }
        if rest == "gp" { return Some(28); }
        if rest == "sp" { return Some(29); }
        if rest == "fp" || rest == "s8" { return Some(30); }
        if rest == "ra" { return Some(31); }
        if rest.starts_with("s") {
            let n: u32 = rest[1..].parse().ok()?;
            if n <= 7 { return Some(16 + n); }
            if n == 8 { return Some(30); }
        }
        if let Ok(n) = rest.parse::<u32>() {
            if n < 32 { return Some(n); }
        }
    }
    if name == "zero" { return Some(0); }
    None
}

fn encode_r(funct: u32, rd: u32, rs: u32, rt: u32, shamt: u32) -> u32 {
    (0 << 26) | (rs << 21) | (rt << 16) | (rd << 11) | (shamt << 6) | funct
}

fn encode_i(opcode: u32, rs: u32, rt: u32, imm: i32) -> u32 {
    let imm = (imm as u32) & 0xFFFF;
    (opcode << 26) | (rs << 21) | (rt << 16) | imm
}

fn encode_j(opcode: u32, target: u32) -> u32 {
    (opcode << 26) | (target & 0x03FFFFFF)
}

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

impl ArchitecturePlugin for MipsPlugin {
    fn name(&self) -> &str {
        "MIPS"
    }

    fn assemble(&self, source: &str) -> ProgramImage {
        let mut bytes = Vec::new();
        let mut source_map = Vec::new();
        let mut errors = Vec::new();
        let mut labels: HashMap<String, u32> = HashMap::new();
        let mut pending_refs: Vec<(usize, String, u32, u8)> = Vec::new(); // offset, label, line_num, type: 0=imm16, 1=target26

        let lines: Vec<&str> = source.lines().collect();
        let mut pc: u32 = 0;

        for (line_no, line) in lines.iter().enumerate() {
            let line_num = (line_no + 1) as u32;
            let line = line
                .split('#')
                .next()
                .unwrap_or(line)
                .trim();
            if line.is_empty() {
                continue;
            }

            let col = (line.find(|c: char| !c.is_whitespace()).unwrap_or(0) + 1) as u32;

            if let Some(idx) = line.find(':') {
                let label = line[..idx].trim().to_string();
                if !label.is_empty() && !label.starts_with('.') {
                    labels.insert(label, pc);
                }
                let rest = line[idx + 1..].trim();
                if rest.is_empty() {
                    continue;
                }
                if let Err(e) = parse_mips_instruction(
                    rest, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                    &mut errors, &mut pending_refs,
                ) {
                    errors.push(e);
                }
                pc += 4;
                continue;
            }

            if let Err(e) = parse_mips_instruction(
                line, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                &mut errors, &mut pending_refs,
            ) {
                errors.push(e);
            }
            pc += 4;
        }

        for (offset, label, line_num, ref_type) in pending_refs {
            if let Some(&target) = labels.get(&label) {
                let insn = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                let patched = if ref_type == 0 {
                    // I-type branch: offset in words = (target - (pc+4)) / 4
                    let pc_at_insn = offset as u32;
                    let pc_next = pc_at_insn + 4;
                    let diff = ((target as i32) - (pc_next as i32)) / 4;
                    (insn & 0xFFFF0000) | ((diff as u32) & 0xFFFF)
                } else {
                    // J-type: target26 = target >> 2 (low 28 bits of addr)
                    (insn & 0xFC000000) | ((target >> 2) & 0x03FFFFFF)
                };
                bytes[offset..offset + 4].copy_from_slice(&patched.to_le_bytes());
            } else {
                errors.push(AssemblerError { line: line_num, column: 1, message: format!("Unknown label: {}", label) });
            }
        }

        ProgramImage {
            entry_pc: labels.get("_start").or(labels.get("main")).copied().unwrap_or(0),
            bytes,
            source_map,
            errors,
        }
    }

    fn reset(&self, _config: &ResetConfig) -> CpuState {
        CpuState {
            pc: 0,
            regs: vec![0u32; 34], // $0-$31, then HI (32), LO (33)
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

        let pc = state.pc;
        let mut regs = state.regs.clone();
        while regs.len() < 34 {
            regs.push(0);
        }

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
                    pipeline_stages: vec![],
                    io_output: None,
                io_input_requested: None,
                };
            }
        };

        let mut undo_log = Vec::new();
        let mut events = vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Alu];

        let opcode = instr >> 26;
        let rs = ((instr >> 21) & 0x1F) as usize;
        let rt = ((instr >> 16) & 0x1F) as usize;
        let rd = ((instr >> 11) & 0x1F) as usize;
        let shamt = (instr >> 6) & 0x1F;
        let funct = instr & 0x3F;
        let imm = ((instr as i32) << 16 >> 16) as u32;
        let target = instr & 0x03FFFFFF;

        let next_pc = pc + 4;

        let pipeline_stages: Vec<PipelineCycleInfo> = if opcode == 0 {
            match funct {
                0x20 => {
                    // ADD
                    let a = regs[rs] as i32;
                    let b = regs[rt] as i32;
                    let result = a.wrapping_add(b) as u32;
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode ADD ${}, ${}, ${}", rd, rs, rt),
                        &format!("ALU: ${} + ${} = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x22 => {
                    // SUB
                    let a = regs[rs] as i32;
                    let b = regs[rt] as i32;
                    let result = a.wrapping_sub(b) as u32;
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode SUB",
                        &format!("ALU: ${} - ${} = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x24 => {
                    // AND
                    let result = regs[rs] & regs[rt];
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode AND",
                        &format!("ALU: ${} & ${} = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x25 => {
                    // OR
                    let result = regs[rs] | regs[rt];
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode OR",
                        &format!("ALU: ${} | ${} = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x2A => {
                    // SLT
                    let a = regs[rs] as i32;
                    let b = regs[rt] as i32;
                    let result = if a < b { 1u32 } else { 0u32 };
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode SLT",
                        &format!("ALU: (${} < ${}) ? 1 : 0 = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x2B => {
                    // SLTU
                    let result = if regs[rs] < regs[rt] { 1u32 } else { 0u32 };
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode SLTU",
                        &format!("ALU: (${} < ${} unsigned) ? 1 : 0 = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x26 => {
                    // XOR
                    let result = regs[rs] ^ regs[rt];
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode XOR",
                        &format!("ALU: ${} ^ ${} = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x27 => {
                    // NOR: rd = ~(rs | rt)
                    let result = !(regs[rs] | regs[rt]);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode NOR",
                        &format!("ALU: ~(${} | ${}) = {}", rs, rt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x00 => {
                    // SLL: rd = rt << shamt
                    let result = regs[rt].wrapping_shl(shamt);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode SLL ${}, ${}, {}", rd, rt, shamt),
                        &format!("ALU: ${} << {} = {}", rt, shamt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x02 => {
                    // SRL: rd = rt >> shamt (logical)
                    let result = regs[rt].wrapping_shr(shamt);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode SRL ${}, ${}, {}", rd, rt, shamt),
                        &format!("ALU: ${} >> {} (logical) = {}", rt, shamt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x03 => {
                    // SRA: rd = (rt as i32) >> shamt (arithmetic)
                    let result = ((regs[rt] as i32).wrapping_shr(shamt)) as u32;
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode SRA ${}, ${}, {}", rd, rt, shamt),
                        &format!("ALU: ${} >> {} (arithmetic) = {}", rt, shamt, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x04 => {
                    // SLLV: rd = rt << (rs & 0x1F)
                    let sh = (regs[rs] & 0x1F) as u32;
                    let result = regs[rt].wrapping_shl(sh);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode SLLV ${}, ${}, ${}", rd, rt, rs),
                        &format!("ALU: ${} << (${} & 0x1F) = {}", rt, rs, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x06 => {
                    // SRLV: rd = rt >> (rs & 0x1F) (logical)
                    let sh = (regs[rs] & 0x1F) as u32;
                    let result = regs[rt].wrapping_shr(sh);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode SRLV",
                        &format!("ALU: ${} >> (${} & 0x1F) = {}", rt, rs, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x07 => {
                    // SRAV: rd = (rt as i32) >> (rs & 0x1F) (arithmetic)
                    let sh = (regs[rs] & 0x1F) as u32;
                    let result = ((regs[rt] as i32).wrapping_shr(sh)) as u32;
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: result });
                        regs[rd] = result;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode SRAV",
                        &format!("ALU: ${} >> (${} & 0x1F) (arith) = {}", rt, rs, result),
                        "NOP",
                        &format!("${} ← {}", rd, result),
                    )
                }
                0x08 => {
                    // JR
                    let target = regs[rs];
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode JR ${}", rs),
                        &format!("PC ← ${} = 0x{:08X}", rs, target),
                        "NOP",
                        "PC updated",
                    )
                }
                0x09 => {
                    // JALR: rd = PC+4, PC = rs
                    let target = regs[rs];
                    let link = next_pc;
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: link });
                        regs[rd] = link;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        &format!("Decode JALR ${}, ${}", rd, rs),
                        &format!("${} ← PC+4, PC ← ${} = 0x{:08X}", rd, rs, target),
                        "NOP",
                        "PC updated",
                    )
                }
                0x10 => {
                    // MFHI: rd = HI
                    let hi = regs.get(32).copied().unwrap_or(0);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: hi });
                        regs[rd] = hi;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode MFHI",
                        &format!("${} ← HI = 0x{:08X}", rd, hi),
                        "NOP",
                        &format!("${} ← {}", rd, hi),
                    )
                }
                0x12 => {
                    // MFLO: rd = LO
                    let lo = regs.get(33).copied().unwrap_or(0);
                    if rd > 0 {
                        undo_log.push(UndoEntry::RegWrite { reg: rd, old_value: regs[rd], new_value: lo });
                        regs[rd] = lo;
                    }
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode MFLO",
                        &format!("${} ← LO = 0x{:08X}", rd, lo),
                        "NOP",
                        &format!("${} ← {}", rd, lo),
                    )
                }
                0x18 => {
                    // MULT: (HI, LO) = rs * rt (signed)
                    let a = regs[rs] as i32;
                    let b = regs[rt] as i32;
                    let product = (a as i64).wrapping_mul(b as i64);
                    let hi = (product >> 32) as u32;
                    let lo = (product & 0xFFFFFFFF) as u32;
                    let old_hi = regs.get(32).copied().unwrap_or(0);
                    let old_lo = regs.get(33).copied().unwrap_or(0);
                    undo_log.push(UndoEntry::RegWrite { reg: 32, old_value: old_hi, new_value: hi });
                    undo_log.push(UndoEntry::RegWrite { reg: 33, old_value: old_lo, new_value: lo });
                    regs[32] = hi;
                    regs[33] = lo;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode MULT",
                        &format!("(HI, LO) ← ${} * ${} (signed)", rs, rt),
                        "NOP",
                        "HI, LO updated",
                    )
                }
                0x19 => {
                    // MULTU: (HI, LO) = rs * rt (unsigned)
                    let a = regs[rs] as u64;
                    let b = regs[rt] as u64;
                    let product = a.wrapping_mul(b);
                    let hi = (product >> 32) as u32;
                    let lo = (product & 0xFFFFFFFF) as u32;
                    let old_hi = regs.get(32).copied().unwrap_or(0);
                    let old_lo = regs.get(33).copied().unwrap_or(0);
                    undo_log.push(UndoEntry::RegWrite { reg: 32, old_value: old_hi, new_value: hi });
                    undo_log.push(UndoEntry::RegWrite { reg: 33, old_value: old_lo, new_value: lo });
                    regs[32] = hi;
                    regs[33] = lo;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode MULTU",
                        &format!("(HI, LO) ← ${} * ${} (unsigned)", rs, rt),
                        "NOP",
                        "HI, LO updated",
                    )
                }
                0x1A => {
                    // DIV: LO = rs/rt (signed), HI = rs%rt (signed)
                    let a = regs[rs] as i32;
                    let b = regs[rt] as i32;
                    let (lo, hi) = if b == 0 {
                        (0u32, regs[rs])
                    } else {
                        let q = a.wrapping_div(b);
                        let r = a.wrapping_rem(b);
                        (q as u32, r as u32)
                    };
                    let old_hi = regs.get(32).copied().unwrap_or(0);
                    let old_lo = regs.get(33).copied().unwrap_or(0);
                    undo_log.push(UndoEntry::RegWrite { reg: 32, old_value: old_hi, new_value: hi });
                    undo_log.push(UndoEntry::RegWrite { reg: 33, old_value: old_lo, new_value: lo });
                    regs[32] = hi;
                    regs[33] = lo;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode DIV",
                        &format!("LO = ${}/${}, HI = ${}%${}", rs, rt, rs, rt),
                        "NOP",
                        "HI, LO updated",
                    )
                }
                0x1B => {
                    // DIVU: LO = rs/rt (unsigned), HI = rs%rt (unsigned)
                    let a = regs[rs];
                    let b = regs[rt];
                    let (lo, hi) = if b == 0 {
                        (0u32, a)
                    } else {
                        (a / b, a % b)
                    };
                    let old_hi = regs.get(32).copied().unwrap_or(0);
                    let old_lo = regs.get(33).copied().unwrap_or(0);
                    undo_log.push(UndoEntry::RegWrite { reg: 32, old_value: old_hi, new_value: hi });
                    undo_log.push(UndoEntry::RegWrite { reg: 33, old_value: old_lo, new_value: lo });
                    regs[32] = hi;
                    regs[33] = lo;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    events.push(TraceEvent::RegWrite);
                    pipeline_5(
                        Some(instr),
                        &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                        "Decode DIVU",
                        &format!("LO = ${}/${}, HI = ${}%${} (unsigned)", rs, rt, rs, rt),
                        "NOP",
                        "HI, LO updated",
                    )
                }
                0x0C => {
                    // SYSCALL: v0=10 exit, v0=1 print_int, v0=11 print_char, v0=5 read_int, v0=8 read_string, v0=12 read_char
                    let v0 = regs[2];
                    let a0 = regs[4];
                    let a1 = regs[5];
                    let (halt, io_out) = match v0 {
                        10 => (true, None),  // exit
                        17 => (true, None),  // exit with value in $a0 (SPIM)
                        4 => {
                            // Print string: a0 = address of null-terminated string
                            let mut s = String::new();
                            let mut addr = a0;
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
                        1 => (false, Some(format!("{}", a0 as i32))),
                        11 => (false, Some((a0 as u8 as char).to_string())),
                        5 => {
                            // Read integer into $v0
                            match input {
                                Some(s) => {
                                    let val = s.trim().parse::<i32>().unwrap_or(0);
                                    if regs.len() > 2 {
                                        undo_log.push(UndoEntry::RegWrite { reg: 2, old_value: regs[2], new_value: val as u32 });
                                        regs[2] = val as u32;
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
                                            PipelineCycleInfo { stage: "Decode".into(), instruction_bits: Some(instr), action: "syscall 5 (read int) – waiting for input".into() },
                                        ],
                                        io_output: None,
                                        io_input_requested: Some(InputRequest { kind: "int".into(), prompt: "Enter an integer".into(), max_length: None }),
                                    };
                                }
                            }
                        }
                        12 => {
                            // Read character into $v0
                            match input {
                                Some(s) => {
                                    let c = s.chars().next().unwrap_or('\0');
                                    if regs.len() > 2 {
                                        undo_log.push(UndoEntry::RegWrite { reg: 2, old_value: regs[2], new_value: c as u32 & 0xFF });
                                        regs[2] = c as u32 & 0xFF;
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
                                            PipelineCycleInfo { stage: "Decode".into(), instruction_bits: Some(instr), action: "syscall 12 (read char) – waiting for input".into() },
                                        ],
                                        io_output: None,
                                        io_input_requested: Some(InputRequest { kind: "char".into(), prompt: "Enter a character".into(), max_length: Some(1) }),
                                    };
                                }
                            }
                        }
                        8 => {
                            // Read string: buffer at $a0, max length $a1
                            match input {
                                Some(s) => {
                                    events.push(TraceEvent::Mem);
                                    let buf_addr = a0;
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
                                            PipelineCycleInfo { stage: "Decode".into(), instruction_bits: Some(instr), action: "syscall 8 (read string) – waiting for input".into() },
                                        ],
                                        io_output: None,
                                        io_input_requested: Some(InputRequest { kind: "string".into(), prompt: "Enter a string".into(), max_length: Some(a1) }),
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
                                error: Some(format!("Unsupported syscall $v0={}", v0)),
                                instruction_bits: Some(instr),
                                pipeline_stages: vec![],
                                io_output: None,
                io_input_requested: None,
                            };
                        }
                    };
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
                    let action_str = if let Some(ref s) = io_out {
                        format!("syscall: print \"{}\"", s.replace('\n', "\\n"))
                    } else if v0 == 17 {
                        format!("syscall 17: exit with value {}", a0)
                    } else {
                        "syscall 10: exit".to_string()
                    };
                    return StepResult {
                        new_state: CpuState { pc: next_pc, regs, halted: halt },
                        events: if halt { vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Halted] } else { events.clone() },
                        undo_log,
                        cycles_added: if halt { 3 } else { 5 },
                        halted: halt,
                        error: None,
                        instruction_bits: Some(instr),
                        pipeline_stages: if halt {
                            pipeline_halt(
                                Some(instr),
                                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                                "Decode: syscall 10 (exit)",
                                "Halted",
                            )
                        } else {
                            pipeline_5(
                                Some(instr),
                                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                                "Decode syscall",
                                &action_str,
                                "NOP",
                                "PC ← PC+4",
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
                        error: Some(format!("Unknown MIPS R-type funct 0x{:02X}", funct)),
                        instruction_bits: Some(instr),
                        pipeline_stages: vec![],
                        io_output: None,
                io_input_requested: None,
                    };
                }
            }
        } else if opcode == 0x0F {
            // LUI: rt = imm << 16
            let imm_upper = (imm & 0xFFFF) << 16;
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: imm_upper });
                regs[rt] = imm_upper;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode LUI ${}, 0x{:04X}", rt, imm & 0xFFFF),
                &format!("ALU: imm << 16 = 0x{:08X}", imm_upper),
                "NOP",
                &format!("${} ← 0x{:08X}", rt, imm_upper),
            )
        } else if opcode == 0x0A {
            // SLTI: rt = (rs < sign_ext(imm)) ? 1 : 0
            let a = regs[rs] as i32;
            let b = imm as i32;
            let result = if a < b { 1u32 } else { 0u32 };
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: result });
                regs[rt] = result;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode SLTI ${}, ${}, {}", rt, rs, imm as i32),
                &format!("ALU: (${} < {}) ? 1 : 0 = {}", rs, imm as i32, result),
                "NOP",
                &format!("${} ← {}", rt, result),
            )
        } else if opcode == 0x0B {
            // SLTIU: rt = (rs < imm unsigned) ? 1 : 0 (imm zero-extended)
            let result = if regs[rs] < imm { 1u32 } else { 0u32 };
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: result });
                regs[rt] = result;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode SLTIU ${}, ${}, {}", rt, rs, imm),
                &format!("ALU: (${} < {} unsigned) ? 1 : 0 = {}", rs, imm, result),
                "NOP",
                &format!("${} ← {}", rt, result),
            )
        } else if opcode == 0x0E {
            // XORI: rt = rs ^ imm (zero-extended)
            let result = regs[rs] ^ imm;
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: result });
                regs[rt] = result;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode XORI ${}, ${}, 0x{:04X}", rt, rs, imm),
                &format!("ALU: ${} ^ 0x{:04X} = {}", rs, imm, result),
                "NOP",
                &format!("${} ← {}", rt, result),
            )
        } else if opcode == 0x08 || opcode == 0x09 {
            // ADDI (0x08) or ADDIU (0x09) - both add rs + sign-extended imm; li pseudo-op uses ADDIU
            let a = regs[rs] as i32;
            let imm32 = imm as i32;
            let result = a.wrapping_add(imm32) as u32;
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: result });
                regs[rt] = result;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            let name = if opcode == 0x09 { "ADDIU" } else { "ADDI" };
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode {} ${}, ${}, {}", name, rt, rs, imm32),
                &format!("ALU: ${} + {} = {}", rs, imm32, result),
                "NOP",
                &format!("${} ← {}", rt, result),
            )
        } else if opcode == 0x20 {
            // LB: load byte, sign-extend
            events.push(TraceEvent::Mem);
            let addr = regs[rs].wrapping_add(imm);
            let val = match mem.read_u8(addr) {
                Ok(v) => (v as i8 as i32) as u32,
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
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: val });
                regs[rt] = val;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode LB ${}, {}(${})", rt, imm as i32, rs),
                &format!("addr = ${} + {} = 0x{:08X}", rs, imm as i32, addr),
                &format!("Mem[0x{:08X}] (byte) = 0x{:02X} → sign-ext → 0x{:08X}", addr, mem.read_u8(addr).unwrap_or(0), val),
                &format!("${} ← 0x{:08X}", rt, val),
            )
        } else if opcode == 0x21 {
            // LH: load halfword, sign-extend
            events.push(TraceEvent::Mem);
            let addr = regs[rs].wrapping_add(imm);
            let val = match mem.read_u16_le(addr) {
                Ok(v) => (v as i16 as i32) as u32,
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
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: val });
                regs[rt] = val;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode LH ${}, {}(${})", rt, imm as i32, rs),
                &format!("addr = ${} + {} = 0x{:08X}", rs, imm as i32, addr),
                &format!("Mem[0x{:08X}] (half) = 0x{:04X} → sign-ext → 0x{:08X}", addr, mem.read_u16_le(addr).unwrap_or(0), val),
                &format!("${} ← 0x{:08X}", rt, val),
            )
        } else if opcode == 0x23 {
            // LW
            events.push(TraceEvent::Mem);
            let addr = regs[rs].wrapping_add(imm);
            let val = match mem.read_u32_le(addr) {
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
            if rt > 0 {
                undo_log.push(UndoEntry::RegWrite { reg: rt, old_value: regs[rt], new_value: val });
                regs[rt] = val;
            }
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode LW ${}, {}(${})", rt, imm as i32, rs),
                &format!("addr = ${} + {} = 0x{:08X}", rs, imm as i32, addr),
                &format!("Mem[0x{:08X}] = {}", addr, val),
                &format!("${} ← {}", rt, val),
            )
        } else if opcode == 0x2B {
            // SW
            events.push(TraceEvent::Mem);
            let addr = regs[rs].wrapping_add(imm);
            let val = regs[rt];
            let old = match mem.write_u32_le(addr, val) {
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
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode SW ${}, {}(${})", rt, imm as i32, rs),
                &format!("addr = ${} + {} = 0x{:08X}", rs, imm as i32, addr),
                &format!("Mem[0x{:08X}] ← {}", addr, val),
                "NOP",
            )
        } else if opcode == 0x28 {
            // SB: store byte
            events.push(TraceEvent::Mem);
            let addr = regs[rs].wrapping_add(imm);
            let val = (regs[rt] & 0xFF) as u8;
            let old = match mem.write_u8(addr, val) {
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
            undo_log.push(UndoEntry::MemWrite { addr, old_value: old, new_value: val });
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_pc });
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode SB ${}, {}(${})", rt, imm as i32, rs),
                &format!("addr = ${} + {} = 0x{:08X}", rs, imm as i32, addr),
                &format!("Mem[0x{:08X}] ← 0x{:02X}", addr, val),
                "NOP",
            )
        } else if opcode == 0x29 {
            // SH: store halfword
            events.push(TraceEvent::Mem);
            let addr = regs[rs].wrapping_add(imm);
            let val = (regs[rt] & 0xFFFF) as u16;
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
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                &format!("Decode SH ${}, {}(${})", rt, imm as i32, rs),
                &format!("addr = ${} + {} = 0x{:08X}", rs, imm as i32, addr),
                &format!("Mem[0x{:08X}] ← 0x{:04X}", addr, val),
                "NOP",
            )
        } else if opcode == 0x04 {
            // BEQ
            let take = regs[rs] == regs[rt];
            let target = pc + 4 + (imm as i32 as u32) * 4;
            let next = if take { target } else { next_pc };
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
            let exec_str = if take {
                format!("BEQ: ${} == ${} → taken, PC ← 0x{:08X}", rs, rt, target)
            } else {
                format!("BEQ: ${} != ${} → not taken", rs, rt)
            };
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                "Decode BEQ",
                &exec_str,
                "NOP",
                "PC updated",
            )
        } else if opcode == 0x05 {
            // BNE
            let take = regs[rs] != regs[rt];
            let target = pc + 4 + (imm as i32 as u32) * 4;
            let next = if take { target } else { next_pc };
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
            let exec_str = if take {
                format!("BNE: ${} != ${} → taken, PC ← 0x{:08X}", rs, rt, target)
            } else {
                format!("BNE: ${} == ${} → not taken", rs, rt)
            };
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                "Decode BNE",
                &exec_str,
                "NOP",
                "PC updated",
            )
        } else if opcode == 0x02 {
            // J
            let target = (pc & 0xF0000000) | (target << 2);
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                "Decode J",
                &format!("PC ← 0x{:08X}", target),
                "NOP",
                "PC updated",
            )
        } else if opcode == 0x03 {
            // JAL
            let target = (pc & 0xF0000000) | (target << 2);
            undo_log.push(UndoEntry::RegWrite { reg: 31, old_value: regs[31], new_value: next_pc });
            regs[31] = next_pc;
            undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
            events.push(TraceEvent::RegWrite);
            pipeline_5(
                Some(instr),
                &format!("Load 0x{:08X} from IMem[PC=0x{:08X}]", instr, pc),
                "Decode JAL",
                &format!("$ra ← PC+4, PC ← 0x{:08X}", target),
                "NOP",
                "$ra, PC updated",
            )
        } else {
            return StepResult {
                new_state: state.clone(),
                events: vec![],
                undo_log: vec![],
                cycles_added: 0,
                halted: false,
                error: Some(format!("Unknown MIPS opcode 0x{:02X} at PC=0x{:08X}", opcode, pc)),
                instruction_bits: Some(instr),
                pipeline_stages: vec![],
                io_output: None,
                io_input_requested: None,
            };
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
                UiBlock { id: "regfile".into(), label: "Registers".into(), x: 185.0, y: 70.0, width: 75.0, height: 45.0 },
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
        let names = [
            "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3",
            "t0", "t1", "t2", "t3", "t4", "t5", "t6", "t7",
            "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7",
            "t8", "t9", "k0", "k1", "gp", "sp", "s8", "ra",
        ];
        let mut reg_names: Vec<String> = (0..32).map(|i| format!("${} ({})", i, names[i])).collect();
        reg_names.push("HI".to_string());
        reg_names.push("LO".to_string());
        RegisterSchema {
            pc_name: "PC".to_string(),
            reg_names,
        }
    }
}

fn parse_mips_instruction(
    line: &str,
    line_num: u32,
    col: u32,
    pc: u32,
    labels: &HashMap<String, u32>,
    bytes: &mut Vec<u8>,
    source_map: &mut Vec<SourceMapEntry>,
    errors: &mut Vec<AssemblerError>,
    pending_refs: &mut Vec<(usize, String, u32, u8)>,
) -> Result<(), AssemblerError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(());
    }

    source_map.push(SourceMapEntry { pc, line: line_num, column: col });

    let mnemonic = tokens[0].to_lowercase();
    let args: Vec<&str> = tokens.iter().skip(1).copied().collect();

    let mut encode = |insn: u32| {
        let offset = bytes.len();
        bytes.extend_from_slice(&insn.to_le_bytes());
        offset
    };

    match mnemonic.as_str() {
        "add" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "add rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x20, rd, rs, rt, 0));
        }
        "sub" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sub rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x22, rd, rs, rt, 0));
        }
        "and" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "and rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x24, rd, rs, rt, 0));
        }
        "or" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "or rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x25, rd, rs, rt, 0));
        }
        "sll" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sll rd, rt, shamt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let shamt: u32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid shamt: {}", args[2]) })?;
            if shamt > 31 {
                return Err(AssemblerError { line: line_num, column: col, message: "shamt must be 0–31".to_string() });
            }
            encode(encode_r(0x00, rd, 0, rt, shamt));
        }
        "srl" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "srl rd, rt, shamt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let shamt: u32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid shamt: {}", args[2]) })?;
            if shamt > 31 {
                return Err(AssemblerError { line: line_num, column: col, message: "shamt must be 0–31".to_string() });
            }
            encode(encode_r(0x02, rd, 0, rt, shamt));
        }
        "sra" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sra rd, rt, shamt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let shamt: u32 = args[2].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid shamt: {}", args[2]) })?;
            if shamt > 31 {
                return Err(AssemblerError { line: line_num, column: col, message: "shamt must be 0–31".to_string() });
            }
            encode(encode_r(0x03, rd, 0, rt, shamt));
        }
        "jr" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "jr rs".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            encode(encode_r(0x08, 0, rs, 0, 0));
        }
        "jalr" => {
            if args.len() < 1 || args.len() > 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "jalr [rd,] rs".to_string() });
            }
            let (rd, rs) = if args.len() == 2 {
                let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
                let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
                (rd, rs)
            } else {
                (31, parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?)
            };
            encode(encode_r(0x09, rd, rs, 0, 0));
        }
        "sllv" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sllv rd, rt, rs".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x04, rd, rs, rt, 0));
        }
        "srlv" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "srlv rd, rt, rs".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x06, rd, rs, rt, 0));
        }
        "srav" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "srav rd, rt, rs".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rs = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x07, rd, rs, rt, 0));
        }
        "mfhi" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "mfhi rd".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            encode(encode_r(0x10, rd, 0, 0, 0));
        }
        "mflo" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "mflo rd".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            encode(encode_r(0x12, rd, 0, 0, 0));
        }
        "mult" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "mult rs, rt".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            encode(encode_r(0x18, 0, rs, rt, 0));
        }
        "multu" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "multu rs, rt".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            encode(encode_r(0x19, 0, rs, rt, 0));
        }
        "div" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "div rs, rt".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            encode(encode_r(0x1A, 0, rs, rt, 0));
        }
        "divu" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "divu rs, rt".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            encode(encode_r(0x1B, 0, rs, rt, 0));
        }
        "syscall" => {
            if !args.is_empty() {
                return Err(AssemblerError { line: line_num, column: col, message: "syscall (no args)".to_string() });
            }
            encode(encode_r(0x0C, 0, 0, 0, 0));
        }
        "addi" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "addi rt, rs, imm".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = parse_mips_imm(args[2]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x08, rs, rt, imm));
        }
        "lui" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "lui rt, imm".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let imm: i32 = parse_mips_imm(args[1]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?;
            encode(encode_i(0x0F, 0, rt, imm)); // rs=0 for LUI
        }
        "slt" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "slt rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x2A, rd, rs, rt, 0));
        }
        "sltu" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sltu rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x2B, rd, rs, rt, 0));
        }
        "nor" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "nor rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x27, rd, rs, rt, 0));
        }
        "xor" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "xor rd, rs, rt".to_string() });
            }
            let rd = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let rt = parse_reg(args[2]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[2]) })?;
            encode(encode_r(0x26, rd, rs, rt, 0));
        }
        "slti" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "slti rt, rs, imm".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = parse_mips_imm(args[2]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x0A, rs, rt, imm));
        }
        "sltiu" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "sltiu rt, rs, imm".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = parse_mips_imm(args[2]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x0B, rs, rt, imm));
        }
        "xori" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "xori rt, rs, imm".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rs = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let imm: i32 = parse_mips_imm(args[2]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[2]) })?;
            encode(encode_i(0x0E, rs, rt, imm));
        }
        "lb" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "lb rt, offset(rs)".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (offset, rs) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x20, rs, rt, offset));
        }
        "lh" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "lh rt, offset(rs)".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (offset, rs) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x21, rs, rt, offset));
        }
        "lw" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "lw rt, offset(rs)".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (offset, rs) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x23, rs, rt, offset));
        }
        "sw" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "sw rt, offset(rs)".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (offset, rs) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x2B, rs, rt, offset));
        }
        "sb" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "sb rt, offset(rs)".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (offset, rs) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x28, rs, rt, offset));
        }
        "sh" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "sh rt, offset(rs)".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let (offset, rs) = parse_offset_base(args[1]).map_err(|e| AssemblerError { line: line_num, column: col, message: e })?;
            encode(encode_i(0x29, rs, rt, offset));
        }
        "beq" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "beq rs, rt, label".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let pos = encode(encode_i(0x04, rs, rt, 0));
            pending_refs.push((pos, args[2].to_string(), line_num, 0));
        }
        "bne" => {
            if args.len() != 3 {
                return Err(AssemblerError { line: line_num, column: col, message: "bne rs, rt, label".to_string() });
            }
            let rs = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let rt = parse_reg(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[1]) })?;
            let pos = encode(encode_i(0x05, rs, rt, 0));
            pending_refs.push((pos, args[2].to_string(), line_num, 0));
        }
        "j" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "j label".to_string() });
            }
            let pos = encode(encode_j(0x02, 0));
            pending_refs.push((pos, args[0].to_string(), line_num, 1));
        }
        "jal" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "jal label".to_string() });
            }
            let pos = encode(encode_j(0x03, 0));
            pending_refs.push((pos, args[0].to_string(), line_num, 1));
        }
        "li" => {
            if args.len() != 2 {
                return Err(AssemblerError { line: line_num, column: col, message: "li rt, imm".to_string() });
            }
            let rt = parse_reg(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: format!("Invalid reg: {}", args[0]) })?;
            let imm: i32 = parse_mips_imm(args[1]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid imm: {}", args[1]) })?;
            encode(encode_i(0x09, 0, rt, imm)); // addiu $rt, $0, imm
        }
        "nop" => {
            encode(encode_r(0, 0, 0, 0, 0));
        }
        _ => return Err(AssemblerError { line: line_num, column: col, message: format!("Unknown instruction: {}", mnemonic) }),
    }

    Ok(())
}

fn parse_mips_imm(s: &str) -> Result<i32, ()> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        i32::from_str_radix(&s[2..], 16).map_err(|_| ())
    } else {
        s.parse().map_err(|_| ())
    }
}

fn parse_offset_base(s: &str) -> Result<(i32, u32), String> {
    let s = s.trim();
    if let Some(p) = s.find('(') {
        let offset_str = s[..p].trim();
        let base = s[p + 1..].trim().trim_end_matches(')');
        let imm: i32 = if offset_str.is_empty() {
            0
        } else {
            offset_str.parse().map_err(|_| format!("Invalid offset: {}", offset_str))?
        };
        let rs = parse_reg(base).ok_or_else(|| format!("Invalid reg: {}", base))?;
        Ok((imm, rs))
    } else {
        Err(format!("Expected offset(rs) format: {}", s))
    }
}
