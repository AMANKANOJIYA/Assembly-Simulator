//! MOS 6502 8-bit CPU: assembler and executor.
//! Registers: A, X, Y, SP, P (status: N V B D I Z C).
//! Subset: LDA, STA, LDX, LDY, STX, STY, TAX, TAY, TXA, TYA, INX, INY, INC, DEC,
//! ADC, SBC, AND, ORA, EOR, CMP, CPX, CPY, JMP, JSR, RTS, BCC, BCS, BNE, BEQ, BMI, BPL, BRK, NOP.

use crate::memory::Memory;
use crate::plugin::*;
use std::collections::HashMap;

pub struct I6502Plugin;

impl I6502Plugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for I6502Plugin {
    fn default() -> Self {
        Self::new()
    }
}

// regs: [A, X, Y, SP, P] - P = status (N=0x80, V=0x40, B=0x10, D=0x08, I=0x04, Z=0x02, C=0x01)
const R_A: usize = 0;
const R_X: usize = 1;
const R_Y: usize = 2;
const R_SP: usize = 3;
const R_P: usize = 4;

const FLAG_N: u32 = 0x80;
const FLAG_V: u32 = 0x40;
const FLAG_B: u32 = 0x10;
const FLAG_D: u32 = 0x08;
const FLAG_I: u32 = 0x04;
const FLAG_Z: u32 = 0x02;
const FLAG_C: u32 = 0x01;

fn set_nz(regs: &mut [u32], val: u8) {
    let p = regs[R_P] & !(FLAG_N | FLAG_Z);
    regs[R_P] = p | (if (val as i8) < 0 { FLAG_N } else { 0 }) | (if val == 0 { FLAG_Z } else { 0 });
}

fn set_c(regs: &mut [u32], c: bool) {
    if c {
        regs[R_P] |= FLAG_C;
    } else {
        regs[R_P] &= !FLAG_C;
    }
}

fn stack_push(mem: &mut Memory, regs: &mut [u32], undo: &mut Vec<UndoEntry>, b: u8) {
    let sp = (regs[R_SP] & 0xFF) as u8;
    let addr = 0x0100u32 | (sp as u32);
    let old = mem.read_u8(addr).unwrap_or(0);
    mem.write_u8(addr, b).ok();
    undo.push(UndoEntry::MemWrite { addr, old_value: old, new_value: b });
    undo.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp.wrapping_sub(1) as u32 });
    regs[R_SP] = sp.wrapping_sub(1) as u32;
}

fn stack_pop(mem: &mut Memory, regs: &mut [u32], undo: &mut Vec<UndoEntry>) -> u8 {
    let sp = (regs[R_SP] & 0xFF) as u8;
    let sp_new = sp.wrapping_add(1);
    undo.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
    regs[R_SP] = sp_new as u32;
    let addr = 0x0100u32 | (sp_new as u32);
    mem.read_u8(addr).unwrap_or(0)
}

fn pipeline_3(instr: Option<u32>, fetch: &str, decode: &str, execute: &str) -> Vec<PipelineCycleInfo> {
    vec![
        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: instr, action: fetch.into() },
        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: instr, action: decode.into() },
        PipelineCycleInfo { stage: "Execute".into(), instruction_bits: instr, action: execute.into() },
    ]
}

fn parse_imm8(s: &str) -> Result<u8, ()> {
    let s = s.trim();
    if s.to_lowercase().starts_with('$') || s.to_lowercase().starts_with("0x") {
        let hex = s.trim_start_matches('$').trim_start_matches("0x");
        u8::from_str_radix(hex, 16).map_err(|_| ())
    } else if s.to_lowercase().starts_with('#') {
        let rest = s[1..].trim();
        if rest.to_lowercase().starts_with('$') {
            u8::from_str_radix(rest.trim_start_matches('$'), 16).map_err(|_| ())
        } else {
            rest.parse::<u8>().map_err(|_| ())
        }
    } else {
        s.parse::<u8>().map_err(|_| ())
    }
}

fn parse_imm16(s: &str) -> Result<u16, ()> {
    let s = s.trim().trim_start_matches('$').trim_start_matches("0x");
    u16::from_str_radix(s, 16).map_err(|_| ())
}

impl ArchitecturePlugin for I6502Plugin {
    fn name(&self) -> &str {
        "6502"
    }

    fn assemble(&self, source: &str) -> ProgramImage {
        let mut bytes = Vec::new();
        let mut source_map = Vec::new();
        let mut errors = Vec::new();
        let mut labels: HashMap<String, u32> = HashMap::new();
        let mut pending_refs: Vec<(usize, String, bool)> = Vec::new(); // (offset, label, is_16bit)

        let lines: Vec<&str> = source.lines().collect();
        let mut pc: u32 = 0;
        let mut origin: u32 = 0;
        let mut origin_set = false;

        for (line_no, line) in lines.iter().enumerate() {
            let line_num = (line_no + 1) as u32;
            let line = line.split(';').next().unwrap_or(line).trim();
            if line.is_empty() {
                continue;
            }
            let col = (line.find(|c: char| !c.is_whitespace()).unwrap_or(0) + 1) as u32;
            let start_len = bytes.len();

            if let Some(idx) = line.find(':') {
                let label = line[..idx].trim().to_string();
                if !label.is_empty() {
                    labels.insert(label, pc);
                }
                let rest = line[idx + 1..].trim();
                if rest.is_empty() {
                    continue;
                }
                if let Err(e) = parse_6502_instruction(
                    rest, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                    &mut errors, &mut pending_refs,
                ) {
                    errors.push(e);
                }
                pc = pc.wrapping_add((bytes.len() - start_len) as u32);
                continue;
            }

            if line.to_uppercase().starts_with(".ORG ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let s = parts[1].trim_start_matches('$').trim_start_matches("0x");
                    if let Ok(addr) = u16::from_str_radix(s, 16) {
                        let addr = addr as u32;
                        if !origin_set && bytes.is_empty() {
                            origin = addr;
                            pc = addr;
                            origin_set = true;
                        } else if addr >= pc {
                            let gap = (addr - pc) as usize;
                            if gap > 0 {
                                bytes.extend(std::iter::repeat(0u8).take(gap));
                            }
                            pc = addr;
                        } else {
                            errors.push(AssemblerError {
                                line: line_num,
                                column: col,
                                message: format!(".ORG cannot move PC backwards (0x{:04X} -> 0x{:04X})", pc, addr),
                            });
                        }
                    }
                }
                continue;
            }

            if let Err(e) = parse_6502_instruction(
                line, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                &mut errors, &mut pending_refs,
            ) {
                errors.push(e);
            }
            pc = pc.wrapping_add((bytes.len() - start_len) as u32);
        }

        for (offset, label, is_16bit) in pending_refs {
            if let Some(&target) = labels.get(&label) {
                if is_16bit {
                    if offset + 1 < bytes.len() {
                        bytes[offset] = (target & 0xFF) as u8;
                        bytes[offset + 1] = (target >> 8) as u8;
                    }
                } else {
                    // Relative branch: offset points to the rel byte; branch is 2 bytes, so base = (PC+2).
                    // PC here is absolute; offset is a byte index into `bytes`, so address = origin + offset.
                    let base = (origin + offset as u32 + 1).wrapping_add(1); // +2
                    let rel = (target as i32) - (base as i32);
                    if rel >= -128 && rel <= 127 && offset < bytes.len() {
                        bytes[offset] = rel as u8;
                    }
                }
            } else {
                errors.push(AssemblerError { line: 0, column: 1, message: format!("Unknown label: {}", label) });
            }
        }

        if origin_set {
            if let Some(&start) = labels.get("_start") {
                if start != origin {
                    errors.push(AssemblerError {
                        line: 0,
                        column: 1,
                        message: format!(
                            "_start label (0x{:04X}) differs from .ORG origin (0x{:04X}); simulator loads+starts at entry_pc only",
                            start, origin
                        ),
                    });
                }
            }
        }

        ProgramImage {
            entry_pc: labels.get("_start").copied().unwrap_or(origin),
            bytes,
            source_map,
            errors,
        }
    }

    fn reset(&self, _config: &ResetConfig) -> CpuState {
        CpuState {
            pc: 0,
            regs: vec![0u32; 5],
            halted: false,
        }
    }

    fn step(&self, state: &CpuState, memory: &[u8], _mode: StepMode, _input: Option<&str>) -> StepResult {
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

        let op = match mem.read_u8(pc) {
            Ok(b) => b,
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
        let instr_bits = op as u32;
        let mut undo_log = Vec::new();
        let mut events = vec![TraceEvent::Fetch, TraceEvent::Decode, TraceEvent::Alu];

        let (next_pc, pipeline_stages, cycles, halted) = match op {
            0xEA => {
                // NOP
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch NOP", "Decode", "NOP"), 1, false)
            }
            0x00 => {
                // BRK - treat as halt
                (
                    pc,
                    pipeline_3(Some(instr_bits), "Fetch BRK", "Decode", "Halt"),
                    1,
                    true,
                )
            }
            0xA9 => {
                // LDA #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: imm as u32 });
                regs[R_A] = imm as u32;
                set_nz(&mut regs, imm);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch LDA #", "Decode", &format!("A ← 0x{:02X}", imm)), 2, false)
            }
            0xAD => {
                // LDA abs
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let addr = lo | (hi << 8);
                let val = mem.read_u8(addr).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: val as u32 });
                regs[R_A] = val as u32;
                set_nz(&mut regs, val);
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 3, pipeline_3(Some(instr_bits), "Fetch LDA abs", "Decode", &format!("A ← Mem[0x{:04X}]", addr)), 3, false)
            }
            0xA5 => {
                // LDA zpg
                let addr = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let val = mem.read_u8(addr).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: val as u32 });
                regs[R_A] = val as u32;
                set_nz(&mut regs, val);
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch LDA zpg", "Decode", &format!("A ← Mem[0x{:02X}]", addr)), 2, false)
            }
            0x8D => {
                // STA abs
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let addr = lo | (hi << 8);
                let val = regs[R_A] as u8;
                let old = mem.read_u8(addr).unwrap_or(0);
                mem.write_u8(addr, val).ok();
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old, new_value: val });
                events.push(TraceEvent::Mem);
                (pc + 3, pipeline_3(Some(instr_bits), "Fetch STA abs", "Decode", &format!("Mem[0x{:04X}] ← A", addr)), 3, false)
            }
            0x85 => {
                // STA zpg
                let addr = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let val = regs[R_A] as u8;
                let old = mem.read_u8(addr).unwrap_or(0);
                mem.write_u8(addr, val).ok();
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old, new_value: val });
                events.push(TraceEvent::Mem);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch STA zpg", "Decode", &format!("Mem[0x{:02X}] ← A", addr)), 2, false)
            }
            0xA2 => {
                // LDX #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_X, old_value: regs[R_X], new_value: imm as u32 });
                regs[R_X] = imm as u32;
                set_nz(&mut regs, imm);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch LDX #", "Decode", &format!("X ← 0x{:02X}", imm)), 2, false)
            }
            0xA0 => {
                // LDY #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_Y, old_value: regs[R_Y], new_value: imm as u32 });
                regs[R_Y] = imm as u32;
                set_nz(&mut regs, imm);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch LDY #", "Decode", &format!("Y ← 0x{:02X}", imm)), 2, false)
            }
            0xAA => {
                // TAX
                let a = regs[R_A] as u8;
                undo_log.push(UndoEntry::RegWrite { reg: R_X, old_value: regs[R_X], new_value: a as u32 });
                regs[R_X] = a as u32;
                set_nz(&mut regs, a);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch TAX", "Decode", "X ← A"), 1, false)
            }
            0xA8 => {
                // TAY
                let a = regs[R_A] as u8;
                undo_log.push(UndoEntry::RegWrite { reg: R_Y, old_value: regs[R_Y], new_value: a as u32 });
                regs[R_Y] = a as u32;
                set_nz(&mut regs, a);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch TAY", "Decode", "Y ← A"), 1, false)
            }
            0x8A => {
                // TXA
                let x = regs[R_X] as u8;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: x as u32 });
                regs[R_A] = x as u32;
                set_nz(&mut regs, x);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch TXA", "Decode", "A ← X"), 1, false)
            }
            0x98 => {
                // TYA
                let y = regs[R_Y] as u8;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: y as u32 });
                regs[R_A] = y as u32;
                set_nz(&mut regs, y);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch TYA", "Decode", "A ← Y"), 1, false)
            }
            0xE8 => {
                // INX
                let x = (regs[R_X] as u8).wrapping_add(1);
                undo_log.push(UndoEntry::RegWrite { reg: R_X, old_value: regs[R_X], new_value: x as u32 });
                regs[R_X] = x as u32;
                set_nz(&mut regs, x);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch INX", "Decode", "X ← X+1"), 1, false)
            }
            0xC8 => {
                // INY
                let y = (regs[R_Y] as u8).wrapping_add(1);
                undo_log.push(UndoEntry::RegWrite { reg: R_Y, old_value: regs[R_Y], new_value: y as u32 });
                regs[R_Y] = y as u32;
                set_nz(&mut regs, y);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch INY", "Decode", "Y ← Y+1"), 1, false)
            }
            0x69 => {
                // ADC #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let c = if (regs[R_P] & FLAG_C) != 0 { 1 } else { 0 };
                let sum = a as u16 + imm as u16 + c;
                let result = (sum & 0xFF) as u8;
                let carry = sum > 0xFF;
                undo_log.push(UndoEntry::RegWrite { reg: R_P, old_value: regs[R_P], new_value: 0 });
                regs[R_P] = (regs[R_P] & !(FLAG_N | FLAG_Z | FLAG_C)) | (if (result as i8) < 0 { FLAG_N } else { 0 }) | (if result == 0 { FLAG_Z } else { 0 }) | (if carry { FLAG_C } else { 0 });
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch ADC #", "Decode", &format!("A ← A + 0x{:02X} + C", imm)), 2, false)
            }
            0xE9 => {
                // SBC #imm  (A = A - imm - (1-C))
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let c = if (regs[R_P] & FLAG_C) != 0 { 1i16 } else { 0i16 };
                let diff = (a as i16) - (imm as i16) - (1 - c);
                let result = (diff as i32 & 0xFF) as u8;
                let carry = diff >= 0;
                let new_p = (regs[R_P] & !(FLAG_N | FLAG_Z | FLAG_C))
                    | (if (result as i8) < 0 { FLAG_N } else { 0 })
                    | (if result == 0 { FLAG_Z } else { 0 })
                    | (if carry { FLAG_C } else { 0 });
                undo_log.push(UndoEntry::RegWrite { reg: R_P, old_value: regs[R_P], new_value: new_p });
                regs[R_P] = new_p;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch SBC #", "Decode", &format!("A ← A - 0x{:02X} - (1-C)", imm)), 2, false)
            }
            0x29 => {
                // AND #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let result = (regs[R_A] as u8) & imm;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                set_nz(&mut regs, result);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch AND #", "Decode", &format!("A ← A & 0x{:02X}", imm)), 2, false)
            }
            0x09 => {
                // ORA #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let result = (regs[R_A] as u8) | imm;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                set_nz(&mut regs, result);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch ORA #", "Decode", &format!("A ← A | 0x{:02X}", imm)), 2, false)
            }
            0x49 => {
                // EOR #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let result = (regs[R_A] as u8) ^ imm;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                set_nz(&mut regs, result);
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch EOR #", "Decode", &format!("A ← A ^ 0x{:02X}", imm)), 2, false)
            }
            0x18 => {
                // CLC
                let new_p = regs[R_P] & !FLAG_C;
                undo_log.push(UndoEntry::RegWrite { reg: R_P, old_value: regs[R_P], new_value: new_p });
                regs[R_P] = new_p;
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch CLC", "Decode", "C ← 0"), 1, false)
            }
            0x38 => {
                // SEC
                let new_p = regs[R_P] | FLAG_C;
                undo_log.push(UndoEntry::RegWrite { reg: R_P, old_value: regs[R_P], new_value: new_p });
                regs[R_P] = new_p;
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch SEC", "Decode", "C ← 1"), 1, false)
            }
            0x48 => {
                // PHA
                let a = regs[R_A] as u8;
                stack_push(&mut mem, &mut regs, &mut undo_log, a);
                events.push(TraceEvent::Mem);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch PHA", "Decode", "Push A"), 3, false)
            }
            0x68 => {
                // PLA
                let v = stack_pop(&mut mem, &mut regs, &mut undo_log);
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: v as u32 });
                regs[R_A] = v as u32;
                set_nz(&mut regs, v);
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch PLA", "Decode", "Pop A"), 4, false)
            }
            0xC9 => {
                // CMP #imm
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let (diff, carry) = a.overflowing_sub(imm);
                let mut p = regs[R_P] & !(FLAG_N | FLAG_Z | FLAG_C);
                if (diff as i8) < 0 {
                    p |= FLAG_N;
                }
                if diff == 0 {
                    p |= FLAG_Z;
                }
                if !carry {
                    p |= FLAG_C;
                }
                undo_log.push(UndoEntry::RegWrite { reg: R_P, old_value: regs[R_P], new_value: p });
                regs[R_P] = p;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch CMP #", "Decode", &format!("A - 0x{:02X}", imm)), 2, false)
            }
            0x4C => {
                // JMP abs
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                (target, pipeline_3(Some(instr_bits), "Fetch JMP", "Decode", &format!("PC ← 0x{:04X}", target)), 3, false)
            }
            0x20 => {
                // JSR
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                let ret = pc + 3;
                let sp = (regs[R_SP] as u8) as u32;
                let push_addr = 0x100 + sp;
                let sp_new = sp.wrapping_sub(1) & 0xFF;
                let old_hi = mem.read_u8(push_addr).unwrap_or(0);
                mem.write_u8(push_addr, (ret >> 8) as u8).ok();
                undo_log.push(UndoEntry::MemWrite { addr: push_addr, old_value: old_hi, new_value: (ret >> 8) as u8 });
                let push_lo = 0x100 + sp_new;
                let old_lo = mem.read_u8(push_lo).unwrap_or(0);
                mem.write_u8(push_lo, (ret & 0xFF) as u8).ok();
                undo_log.push(UndoEntry::MemWrite { addr: push_lo, old_value: old_lo, new_value: (ret & 0xFF) as u8 });
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
                regs[R_SP] = sp_new as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                events.push(TraceEvent::Mem);
                (target, pipeline_3(Some(instr_bits), "Fetch JSR", "Decode", &format!("Push return, PC ← 0x{:04X}", target)), 3, false)
            }
            0x60 => {
                // RTS
                let sp = (regs[R_SP] as u8) as u32;
                let sp1 = (sp + 1) & 0xFF;
                let sp2 = (sp + 2) & 0xFF;
                let lo = mem.read_u8(0x100 + sp1).unwrap_or(0) as u32;
                let hi = mem.read_u8(0x100 + sp2).unwrap_or(0) as u32;
                let target = (lo | (hi << 8)).wrapping_add(1) & 0xFFFF;
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: (sp + 2) as u32 });
                regs[R_SP] = (sp + 2) as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                events.push(TraceEvent::Mem);
                (target, pipeline_3(Some(instr_bits), "Fetch RTS", "Decode", &format!("PC ← 0x{:04X}", target)), 1, false)
            }
            0x90 => {
                // BCC rel
                let rel = mem.read_u8(pc + 1).unwrap_or(0) as i8;
                let take = (regs[R_P] & FLAG_C) == 0;
                let next = if take { (pc as i32 + 2 + rel as i32) as u32 & 0xFFFF } else { pc + 2 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                (next, pipeline_3(Some(instr_bits), "Fetch BCC", "Decode", if take { "Taken" } else { "Not taken" }), 2, false)
            }
            0xB0 => {
                // BCS rel
                let rel = mem.read_u8(pc + 1).unwrap_or(0) as i8;
                let take = (regs[R_P] & FLAG_C) != 0;
                let next = if take { (pc as i32 + 2 + rel as i32) as u32 & 0xFFFF } else { pc + 2 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                (next, pipeline_3(Some(instr_bits), "Fetch BCS", "Decode", if take { "Taken" } else { "Not taken" }), 2, false)
            }
            0xD0 => {
                // BNE rel
                let rel = mem.read_u8(pc + 1).unwrap_or(0) as i8;
                let take = (regs[R_P] & FLAG_Z) == 0;
                let next = if take { (pc as i32 + 2 + rel as i32) as u32 & 0xFFFF } else { pc + 2 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                (next, pipeline_3(Some(instr_bits), "Fetch BNE", "Decode", if take { "Taken" } else { "Not taken" }), 2, false)
            }
            0xF0 => {
                // BEQ rel
                let rel = mem.read_u8(pc + 1).unwrap_or(0) as i8;
                let take = (regs[R_P] & FLAG_Z) != 0;
                let next = if take { (pc as i32 + 2 + rel as i32) as u32 & 0xFFFF } else { pc + 2 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                (next, pipeline_3(Some(instr_bits), "Fetch BEQ", "Decode", if take { "Taken" } else { "Not taken" }), 2, false)
            }
            _ => {
                return StepResult {
                    new_state: state.clone(),
                    events: vec![],
                    undo_log: vec![],
                    cycles_added: 0,
                    halted: false,
                    error: Some(format!("Unknown 6502 opcode 0x{:02X} at PC=0x{:04X}", op, pc)),
                    instruction_bits: Some(instr_bits),
                    pipeline_stages: vec![],
                    io_output: None,
                    io_input_requested: None,
                };
            }
        };

        let next_pc_final = if halted { pc } else { next_pc };

        StepResult {
            new_state: CpuState { pc: next_pc_final, regs, halted },
            events,
            undo_log,
            cycles_added: cycles,
            halted,
            error: None,
            instruction_bits: Some(instr_bits),
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
                UiBlock { id: "regfile".into(), label: "A X Y SP P".into(), x: 185.0, y: 70.0, width: 85.0, height: 45.0 },
                UiBlock { id: "alu".into(), label: "ALU".into(), x: 285.0, y: 45.0, width: 70.0, height: 45.0 },
                UiBlock { id: "dm".into(), label: "Data Mem".into(), x: 285.0, y: 105.0, width: 75.0, height: 45.0 },
                UiBlock { id: "control".into(), label: "Control".into(), x: 10.0, y: 70.0, width: 115.0, height: 45.0 },
            ],
            connections: vec![
                UiConnection { from: "pc".into(), to: "im".into() },
                UiConnection { from: "im".into(), to: "ir".into() },
                UiConnection { from: "ir".into(), to: "regfile".into() },
                UiConnection { from: "ir".into(), to: "alu".into() },
                UiConnection { from: "regfile".into(), to: "alu".into() },
                UiConnection { from: "alu".into(), to: "regfile".into() },
                UiConnection { from: "ir".into(), to: "control".into() },
            ],
        }
    }

    fn register_schema(&self) -> RegisterSchema {
        RegisterSchema {
            pc_name: "PC".to_string(),
            reg_names: vec!["A".into(), "X".into(), "Y".into(), "SP".into(), "P (NV-BDIZC)".into()],
        }
    }
}

fn parse_6502_instruction(
    line: &str,
    line_num: u32,
    col: u32,
    pc: u32,
    labels: &HashMap<String, u32>,
    bytes: &mut Vec<u8>,
    source_map: &mut Vec<SourceMapEntry>,
    errors: &mut Vec<AssemblerError>,
    pending_refs: &mut Vec<(usize, String, bool)>,
) -> Result<(), AssemblerError> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }
    let mnemonic = parts[0].to_uppercase();
    let arg = if parts.len() > 1 { parts[1].trim() } else { "" };

    let start = bytes.len();
    source_map.push(SourceMapEntry { pc, line: line_num, column: col });

    match mnemonic.as_str() {
        "NOP" => bytes.push(0xEA),
        "BRK" => bytes.push(0x00),
        "LDA" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid LDA immediate".into() })?;
                bytes.push(0xA9);
                bytes.push(imm);
            } else if arg.starts_with('$') {
                let hex = arg.trim_start_matches('$');
                if let Ok(addr) = u16::from_str_radix(hex, 16) {
                    if addr < 0x100 {
                        bytes.push(0xA5);
                        bytes.push((addr & 0xFF) as u8);
                    } else {
                        bytes.push(0xAD);
                        bytes.push((addr & 0xFF) as u8);
                        bytes.push((addr >> 8) as u8);
                    }
                } else {
                    bytes.push(0xAD);
                    let off = bytes.len();
                    bytes.push(0);
                    bytes.push(0);
                    resolve_6502_addr(arg, labels, pending_refs, off, true)?;
                }
            } else {
                bytes.push(0xAD);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_6502_addr(arg, labels, pending_refs, off, true)?;
            }
        }
        "STA" => {
            if arg.starts_with('$') {
                let hex = arg.trim_start_matches('$');
                if let Ok(addr) = u16::from_str_radix(hex, 16) {
                    if addr < 0x100 {
                        bytes.push(0x85);
                        bytes.push((addr & 0xFF) as u8);
                    } else {
                        bytes.push(0x8D);
                        bytes.push((addr & 0xFF) as u8);
                        bytes.push((addr >> 8) as u8);
                    }
                } else {
                    bytes.push(0x8D);
                    let off = bytes.len();
                    bytes.push(0);
                    bytes.push(0);
                    resolve_6502_addr(arg, labels, pending_refs, off, true)?;
                }
            } else {
                bytes.push(0x8D);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_6502_addr(arg, labels, pending_refs, off, true)?;
            }
        }
        "LDX" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid LDX immediate".into() })?;
                bytes.push(0xA2);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "LDX #imm only".into() });
            }
        }
        "LDY" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid LDY immediate".into() })?;
                bytes.push(0xA0);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "LDY #imm only".into() });
            }
        }
        "TAX" => bytes.push(0xAA),
        "TAY" => bytes.push(0xA8),
        "TXA" => bytes.push(0x8A),
        "TYA" => bytes.push(0x98),
        "INX" => bytes.push(0xE8),
        "INY" => bytes.push(0xC8),
        "ADC" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid ADC immediate".into() })?;
                bytes.push(0x69);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "ADC #imm only".into() });
            }
        }
        "SBC" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid SBC immediate".into() })?;
                bytes.push(0xE9);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "SBC #imm only".into() });
            }
        }
        "ORA" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid ORA immediate".into() })?;
                bytes.push(0x09);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "ORA #imm only".into() });
            }
        }
        "EOR" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid EOR immediate".into() })?;
                bytes.push(0x49);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "EOR #imm only".into() });
            }
        }
        "AND" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid AND immediate".into() })?;
                bytes.push(0x29);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "AND #imm only".into() });
            }
        }
        "CMP" => {
            if arg.starts_with('#') {
                let imm = parse_imm8(arg).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid CMP immediate".into() })?;
                bytes.push(0xC9);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "CMP #imm only".into() });
            }
        }
        "JMP" => {
            if let Ok(addr) = parse_imm16(arg.trim_start_matches('$')) {
                bytes.push(0x4C);
                bytes.push((addr & 0xFF) as u8);
                bytes.push((addr >> 8) as u8);
            } else {
                bytes.push(0x4C);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_6502_addr(arg, labels, pending_refs, off, true)?;
            }
        }
        "JSR" => {
            bytes.push(0x20);
            let off = bytes.len();
            bytes.push(0);
            bytes.push(0);
            resolve_6502_addr(arg, labels, pending_refs, off, true)?;
        }
        "RTS" => bytes.push(0x60),
        "CLC" => bytes.push(0x18),
        "SEC" => bytes.push(0x38),
        "PHA" => bytes.push(0x48),
        "PLA" => bytes.push(0x68),
        "BCC" | "BCS" | "BNE" | "BEQ" => {
            let op = match mnemonic.as_str() {
                "BCC" => 0x90,
                "BCS" => 0xB0,
                "BNE" => 0xD0,
                _ => 0xF0,
            };
            bytes.push(op);
            let off = bytes.len();
            bytes.push(0);
            resolve_6502_addr(arg, labels, pending_refs, off, false)?;
        }
        _ => return Err(AssemblerError { line: line_num, column: col, message: format!("Unknown mnemonic: {}", mnemonic) }),
    }
    Ok(())
}

fn resolve_6502_addr(
    s: &str,
    labels: &HashMap<String, u32>,
    pending_refs: &mut Vec<(usize, String, bool)>,
    offset: usize,
    is_16bit: bool,
) -> Result<(), AssemblerError> {
    let s = s.trim().trim_start_matches('$');
    if let Ok(v) = u16::from_str_radix(s, 16) {
        return Ok(());
    }
    if labels.contains_key(s) {
        pending_refs.push((offset, s.to_string(), is_16bit));
        return Ok(());
    }
    pending_refs.push((offset, s.to_string(), is_16bit));
    Ok(())
}
