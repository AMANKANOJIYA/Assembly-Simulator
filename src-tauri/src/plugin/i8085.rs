//! Intel 8085 8-bit microprocessor: assembler and executor.
//! Registers: A, B, C, D, E, H, L, SP; flags S, Z, AC, P, C.
//! Full documented Intel 8085 instruction set (including IN/OUT, RST, EI/DI; SIM/RIM treated as NOP).

use crate::memory::Memory;
use crate::plugin::*;
use std::collections::HashMap;

pub struct I8085Plugin;

impl I8085Plugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for I8085Plugin {
    fn default() -> Self {
        Self::new()
    }
}

// 8085 regs: [A, B, C, D, E, H, L, SP_lo, SP_hi] - we store SP in one u32 (low 16 bits), flags in reg 8
// Actually: regs[0]=A, regs[1]=B..regs[6]=L, regs[7]=SP (16-bit), regs[8]=flags (S Z AC P C = bits 4,3,2,1,0)
const R_A: usize = 0;
const R_B: usize = 1;
const R_C: usize = 2;
const R_D: usize = 3;
const R_E: usize = 4;
const R_H: usize = 5;
const R_L: usize = 6;
const R_SP: usize = 7;
const R_FLAGS: usize = 8;

const FLAG_C: u32 = 1;
const FLAG_P: u32 = 2;
const FLAG_AC: u32 = 4;
const FLAG_Z: u32 = 8;
const FLAG_S: u32 = 16;

fn parity_even(x: u8) -> bool {
    (x.count_ones() % 2) == 0
}

fn set_szp(flags: &mut u32, result: u8) {
    *flags &= !(FLAG_S | FLAG_Z | FLAG_P);
    if (result as i8) < 0 {
        *flags |= FLAG_S;
    }
    if result == 0 {
        *flags |= FLAG_Z;
    }
    if parity_even(result) {
        *flags |= FLAG_P;
    }
}

fn set_flags_add(flags: &mut u32, a: u8, b: u8, carry_in: u8, result: u8) {
    *flags &= !(FLAG_S | FLAG_Z | FLAG_P | FLAG_AC | FLAG_C);
    set_szp(flags, result);
    let sum = (a as u16) + (b as u16) + (carry_in as u16);
    if sum > 0xFF {
        *flags |= FLAG_C;
    }
    let ac = ((a & 0x0F) as u16) + ((b & 0x0F) as u16) + (carry_in as u16) > 0x0F;
    if ac {
        *flags |= FLAG_AC;
    }
}

fn set_flags_sub(flags: &mut u32, a: u8, b: u8, borrow_in: u8, result: u8) {
    *flags &= !(FLAG_S | FLAG_Z | FLAG_P | FLAG_AC | FLAG_C);
    set_szp(flags, result);
    // 8085 carry flag is set on borrow for subtraction
    let diff = (a as i16) - (b as i16) - (borrow_in as i16);
    if diff < 0 {
        *flags |= FLAG_C;
    }
    let ac = ((a & 0x0F) as i16) - ((b & 0x0F) as i16) - (borrow_in as i16) < 0;
    if ac {
        *flags |= FLAG_AC;
    }
}

#[allow(dead_code)]
fn pack_psw(a: u8, f: u32) -> (u8, u8) {
    let packed =
        (if (f & FLAG_S) != 0 { 0x80 } else { 0 }) |
        (if (f & FLAG_Z) != 0 { 0x40 } else { 0 }) |
        (if (f & FLAG_AC) != 0 { 0x10 } else { 0 }) |
        (if (f & FLAG_P) != 0 { 0x04 } else { 0 }) |
        0x02 |
        (if (f & FLAG_C) != 0 { 0x01 } else { 0 });
    (a, packed)
}

#[allow(dead_code)]
fn unpack_psw(packed_flags: u8) -> u32 {
    let mut f: u32 = 0;
    if (packed_flags & 0x80) != 0 { f |= FLAG_S; }
    if (packed_flags & 0x40) != 0 { f |= FLAG_Z; }
    if (packed_flags & 0x10) != 0 { f |= FLAG_AC; }
    if (packed_flags & 0x04) != 0 { f |= FLAG_P; }
    if (packed_flags & 0x01) != 0 { f |= FLAG_C; }
    f
}

fn reg_name(idx: usize) -> &'static str {
    match idx {
        0 => "A",
        1 => "B",
        2 => "C",
        3 => "D",
        4 => "E",
        5 => "H",
        6 => "L",
        _ => "?",
    }
}

fn pipeline_3(instr: Option<u32>, fetch: &str, decode: &str, execute: &str) -> Vec<PipelineCycleInfo> {
    vec![
        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: instr, action: fetch.into() },
        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: instr, action: decode.into() },
        PipelineCycleInfo { stage: "Execute".into(), instruction_bits: instr, action: execute.into() },
    ]
}

fn parse_reg_8085(s: &str) -> Option<usize> {
    let s = s.trim().to_uppercase();
    match s.as_str() {
        "A" => Some(R_A),
        "B" => Some(R_B),
        "C" => Some(R_C),
        "D" => Some(R_D),
        "E" => Some(R_E),
        "H" => Some(R_H),
        "L" => Some(R_L),
        "M" => Some(6),
        _ => None,
    }
}

fn parse_imm8(s: &str) -> Result<u8, ()> {
    let s = s.trim();
    if s.to_lowercase().starts_with("0x") || s.to_lowercase().starts_with('x') {
        let hex = s.trim_start_matches(|c| c == 'x' || c == 'X').trim_start_matches("0x");
        u8::from_str_radix(hex, 16).map_err(|_| ())
    } else {
        s.parse::<u8>().map_err(|_| ())
    }
}

fn parse_imm16(s: &str) -> Result<u16, ()> {
    let s = s.trim();
    if s.to_lowercase().starts_with("0x") || s.to_lowercase().starts_with('x') {
        let hex = s.trim_start_matches(|c| c == 'x' || c == 'X').trim_start_matches("0x");
        u16::from_str_radix(hex, 16).map_err(|_| ())
    } else {
        s.parse::<u16>().map_err(|_| ())
    }
}

impl ArchitecturePlugin for I8085Plugin {
    fn name(&self) -> &str {
        "8085"
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
                if let Err(e) = parse_8085_instruction(
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
                    if let Ok(addr) = parse_imm16(parts[1]) {
                        let addr = addr as u32;
                        if !origin_set && bytes.is_empty() {
                            origin = addr;
                            pc = addr;
                            origin_set = true;
                        } else if addr >= pc {
                            // pad gaps so bytes map 1:1 to memory addresses from `origin`
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

            if let Err(e) = parse_8085_instruction(
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
                    let lo = (target & 0xFF) as u8;
                    let hi = (target >> 8) as u8;
                    if offset + 1 < bytes.len() {
                        bytes[offset] = lo;
                        bytes[offset + 1] = hi;
                    }
                } else {
                    // 8085 doesn't have PC-relative branches; keep this for any future pseudo/rel encoding.
                    // Compute relative from the address of the rel byte (origin + offset).
                    let rel = (target as i32) - ((origin + offset as u32) as i32);
                    if offset < bytes.len() {
                        bytes[offset] = (rel as i16 as u16 & 0xFF) as u8;
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
            regs: vec![0u32; 9],
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

        let get_sp = |r: &[u32]| (r[R_SP] & 0xFFFF) as u32;
        let set_sp = |r: &mut [u32], v: u16| r[R_SP] = v as u32;
        let flags = regs[R_FLAGS];

        let get_hl = |r: &[u32]| -> u16 { ((r[R_H] as u16) << 8) | (r[R_L] as u16) };
        let set_hl = |r: &mut [u32], v: u16| {
            r[R_H] = ((v >> 8) & 0xFF) as u32;
            r[R_L] = (v & 0xFF) as u32;
        };
        let read_r = |mem: &Memory, r: &[u32], code: usize| -> u8 {
            if code == 6 {
                let addr = get_hl(r) as u32;
                mem.read_u8(addr).unwrap_or(0)
            } else {
                r[code] as u8
            }
        };
        let write_r = |mem: &mut Memory, r: &mut [u32], undo: &mut Vec<UndoEntry>, code: usize, v: u8| {
            if code == 6 {
                let addr = get_hl(r) as u32;
                let old = mem.read_u8(addr).unwrap_or(0);
                mem.write_u8(addr, v).ok();
                undo.push(UndoEntry::MemWrite { addr, old_value: old, new_value: v });
            } else {
                undo.push(UndoEntry::RegWrite { reg: code, old_value: r[code], new_value: v as u32 });
                r[code] = v as u32;
            }
        };

        let (next_pc, pipeline_stages, cycles) = match op {
            0x00 => {
                // NOP
                (
                    pc + 1,
                    pipeline_3(Some(instr_bits), "Fetch NOP", "Decode", "NOP"),
                    1,
                )
            }
            0x76 => {
                // HLT
                (
                    pc,
                    pipeline_3(Some(instr_bits), "Fetch HLT", "Decode", "Halt"),
                    1,
                )
            }
            _ if op >= 0x40 && op <= 0x7F => {
                // MOV r, r'  (0x40 | dst<<3 | src), M=6
                let dst = ((op >> 3) & 7) as usize;
                let src = (op & 7) as usize;
                let val = read_r(&mem, &regs, src);
                write_r(&mut mem, &mut regs, &mut undo_log, dst, val);
                events.push(TraceEvent::RegWrite);
                (
                    pc + 1,
                    pipeline_3(
                        Some(instr_bits),
                        &format!("Fetch 0x{:02X}", op),
                        "Decode MOV",
                        &format!("MOV: {} ← 0x{:02X}", if dst == 6 { "M" } else { reg_name(dst) }, val),
                    ),
                    1,
                )
            }
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                // MVI r, d8
                let r = ((op >> 3) & 7) as usize;
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                write_r(&mut mem, &mut regs, &mut undo_log, r, imm);
                events.push(TraceEvent::RegWrite);
                (
                    pc + 2,
                    pipeline_3(Some(instr_bits), "Fetch MVI", "Decode", &format!("Load 0x{:02X}", imm)),
                    2,
                )
            }
            0x3A => {
                // LDA addr
                let addr_lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let addr_hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let addr = addr_lo | (addr_hi << 8);
                let val = mem.read_u8(addr).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: val as u32 });
                regs[R_A] = val as u32;
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (
                    pc + 3,
                    pipeline_3(Some(instr_bits), "Fetch LDA", "Decode", &format!("A ← Mem[0x{:04X}] = 0x{:02X}", addr, val)),
                    3,
                )
            }
            0x0A | 0x1A => {
                // LDAX B/D: A <- (BC) or (DE)
                let addr = if op == 0x0A {
                    (((regs[R_B] as u16) << 8) | (regs[R_C] as u16)) as u32
                } else {
                    (((regs[R_D] as u16) << 8) | (regs[R_E] as u16)) as u32
                };
                let val = mem.read_u8(addr).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: val as u32 });
                regs[R_A] = val as u32;
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch LDAX", "Decode", &format!("A ← Mem[0x{:04X}]=0x{:02X}", addr, val)), 1)
            }
            0x32 => {
                // STA addr
                let addr_lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let addr_hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let addr = addr_lo | (addr_hi << 8);
                let val = regs[R_A] as u8;
                let old = mem.read_u8(addr).unwrap_or(0);
                mem.write_u8(addr, val).ok();
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old, new_value: val });
                events.push(TraceEvent::Mem);
                (
                    pc + 3,
                    pipeline_3(Some(instr_bits), "Fetch STA", "Decode", &format!("Mem[0x{:04X}] ← 0x{:02X}", addr, val)),
                    3,
                )
            }
            0x02 | 0x12 => {
                // STAX B/D: (BC) or (DE) <- A
                let addr = if op == 0x02 {
                    (((regs[R_B] as u16) << 8) | (regs[R_C] as u16)) as u32
                } else {
                    (((regs[R_D] as u16) << 8) | (regs[R_E] as u16)) as u32
                };
                let val = regs[R_A] as u8;
                let old = mem.read_u8(addr).unwrap_or(0);
                mem.write_u8(addr, val).ok();
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old, new_value: val });
                events.push(TraceEvent::Mem);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch STAX", "Decode", &format!("Mem[0x{:04X}] ← 0x{:02X}", addr, val)), 1)
            }
            0x80..=0x87 => {
                // ADD r
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let result = a.wrapping_add(b);
                let mut f = regs[R_FLAGS];
                set_flags_add(&mut f, a, b, 0, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (
                    pc + 1,
                    pipeline_3(Some(instr_bits), "Fetch ADD", "Decode", &format!("A ← A + 0x{:02X} = 0x{:02X}", b, result)),
                    1,
                )
            }
            0xC6 => {
                // ADI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let mut f = regs[R_FLAGS];
                let result = a.wrapping_add(imm);
                set_flags_add(&mut f, a, imm, 0, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch ADI", "Decode", &format!("A ← A + 0x{:02X}", imm)), 2)
            }
            0x88..=0x8F => {
                // ADC r
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let cin = if (regs[R_FLAGS] & FLAG_C) != 0 { 1u8 } else { 0u8 };
                let result = a.wrapping_add(b).wrapping_add(cin);
                let mut f = regs[R_FLAGS];
                set_flags_add(&mut f, a, b, cin, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch ADC", "Decode", &format!("A ← A + {} + C", if r == 6 { "M" } else { reg_name(r) })), 1)
            }
            0xCE => {
                // ACI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let cin = if (regs[R_FLAGS] & FLAG_C) != 0 { 1u8 } else { 0u8 };
                let result = a.wrapping_add(imm).wrapping_add(cin);
                let mut f = regs[R_FLAGS];
                set_flags_add(&mut f, a, imm, cin, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch ACI", "Decode", "A ← A + d8 + C"), 2)
            }
            0x90..=0x97 => {
                // SUB r
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let result = a.wrapping_sub(b);
                let mut f = regs[R_FLAGS];
                set_flags_sub(&mut f, a, b, 0, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch SUB", "Decode", &format!("A ← A - 0x{:02X}", b)), 1)
            }
            0xD6 => {
                // SUI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let result = a.wrapping_sub(imm);
                let mut f = regs[R_FLAGS];
                set_flags_sub(&mut f, a, imm, 0, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch SUI", "Decode", &format!("A ← A - 0x{:02X}", imm)), 2)
            }
            0x98..=0x9F => {
                // SBB r (A = A - r - C)
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let bin = if (regs[R_FLAGS] & FLAG_C) != 0 { 1u8 } else { 0u8 };
                let result = a.wrapping_sub(b).wrapping_sub(bin);
                let mut f = regs[R_FLAGS];
                set_flags_sub(&mut f, a, b, bin, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch SBB", "Decode", "A ← A - r - C"), 1)
            }
            0xDE => {
                // SBI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let bin = if (regs[R_FLAGS] & FLAG_C) != 0 { 1u8 } else { 0u8 };
                let result = a.wrapping_sub(imm).wrapping_sub(bin);
                let mut f = regs[R_FLAGS];
                set_flags_sub(&mut f, a, imm, bin, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch SBI", "Decode", "A ← A - d8 - C"), 2)
            }
            0xA0..=0xA7 => {
                // ANA r (AND)
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let result = a & b;
                let mut f = regs[R_FLAGS] & !(FLAG_S | FLAG_Z | FLAG_P | FLAG_C | FLAG_AC);
                // 8085: ANA clears carry, sets AC
                f |= FLAG_AC;
                set_szp(&mut f, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch ANA", "Decode", "A ← A & r"), 1)
            }
            0xE6 => {
                // ANI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let result = a & imm;
                let mut f = regs[R_FLAGS] & !(FLAG_S | FLAG_Z | FLAG_P | FLAG_C | FLAG_AC);
                f |= FLAG_AC;
                set_szp(&mut f, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch ANI", "Decode", "A ← A & d8"), 2)
            }
            0xA8..=0xAF => {
                // XRA r (XOR)
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let result = a ^ b;
                let mut f = regs[R_FLAGS] & !(FLAG_S | FLAG_Z | FLAG_P | FLAG_C | FLAG_AC);
                set_szp(&mut f, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch XRA", "Decode", "A ← A ^ r"), 1)
            }
            0xEE => {
                // XRI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let result = a ^ imm;
                let mut f = regs[R_FLAGS] & !(FLAG_S | FLAG_Z | FLAG_P | FLAG_C | FLAG_AC);
                set_szp(&mut f, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch XRI", "Decode", "A ← A ^ d8"), 2)
            }
            0xB0..=0xB7 => {
                // ORA r (OR)
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let result = a | b;
                let mut f = regs[R_FLAGS] & !(FLAG_S | FLAG_Z | FLAG_P | FLAG_C | FLAG_AC);
                set_szp(&mut f, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch ORA", "Decode", "A ← A | r"), 1)
            }
            0xF6 => {
                // ORI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let result = a | imm;
                let mut f = regs[R_FLAGS] & !(FLAG_S | FLAG_Z | FLAG_P | FLAG_C | FLAG_AC);
                set_szp(&mut f, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch ORI", "Decode", "A ← A | d8"), 2)
            }
            0xB8..=0xBF => {
                // CMP r (A - r, flags only)
                let r = (op & 7) as usize;
                let b = read_r(&mem, &regs, r);
                let a = regs[R_A] as u8;
                let result = a.wrapping_sub(b);
                let mut f = regs[R_FLAGS];
                set_flags_sub(&mut f, a, b, 0, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch CMP", "Decode", "Compare A with r"), 1)
            }
            0xFE => {
                // CPI d8
                let imm = mem.read_u8(pc + 1).unwrap_or(0);
                let a = regs[R_A] as u8;
                let result = a.wrapping_sub(imm);
                let mut f = regs[R_FLAGS];
                set_flags_sub(&mut f, a, imm, 0, result);
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch CPI", "Decode", "Compare A with d8"), 2)
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                // INR r / INR M
                let r = ((op >> 3) & 7) as usize;
                let oldv = read_r(&mem, &regs, r);
                let result = oldv.wrapping_add(1);
                // INR affects SZP and AC, not C
                let mut f = regs[R_FLAGS] & FLAG_C;
                set_szp(&mut f, result);
                if ((oldv & 0x0F) + 1) > 0x0F {
                    f |= FLAG_AC;
                }
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                write_r(&mut mem, &mut regs, &mut undo_log, r, result);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch INR", "Decode", &format!("{} ← {}+1", if r == 6 { "M" } else { reg_name(r) }, if r == 6 { "M" } else { reg_name(r) })), 1)
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                // DCR r / DCR M
                let r = ((op >> 3) & 7) as usize;
                let oldv = read_r(&mem, &regs, r);
                let result = oldv.wrapping_sub(1);
                let mut f = regs[R_FLAGS] & FLAG_C;
                set_szp(&mut f, result);
                if ((oldv & 0x0F) as i16 - 1) < 0 {
                    f |= FLAG_AC;
                }
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                write_r(&mut mem, &mut regs, &mut undo_log, r, result);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch DCR", "Decode", &format!("{} ← {}-1", if r == 6 { "M" } else { reg_name(r) }, if r == 6 { "M" } else { reg_name(r) })), 1)
            }
            0x03 | 0x13 | 0x23 | 0x33 => {
                // INX rp / INX SP
                let rp = (op >> 4) & 0x3;
                match rp {
                    0 => { // BC
                        let v = (((regs[R_B] as u16) << 8) | (regs[R_C] as u16)).wrapping_add(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_B, old_value: regs[R_B], new_value: (v >> 8) as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_C, old_value: regs[R_C], new_value: (v & 0xFF) as u32 });
                        regs[R_B] = (v >> 8) as u32; regs[R_C] = (v & 0xFF) as u32;
                    }
                    1 => { // DE
                        let v = (((regs[R_D] as u16) << 8) | (regs[R_E] as u16)).wrapping_add(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_D, old_value: regs[R_D], new_value: (v >> 8) as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_E, old_value: regs[R_E], new_value: (v & 0xFF) as u32 });
                        regs[R_D] = (v >> 8) as u32; regs[R_E] = (v & 0xFF) as u32;
                    }
                    2 => { // HL
                        let v = get_hl(&regs).wrapping_add(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: (v >> 8) as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: (v & 0xFF) as u32 });
                        set_hl(&mut regs, v);
                    }
                    _ => {
                        let v = (get_sp(&regs) as u16).wrapping_add(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: v as u32 });
                        regs[R_SP] = v as u32;
                    }
                }
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch INX", "Decode", "rp ← rp+1"), 1)
            }
            0x0B | 0x1B | 0x2B | 0x3B => {
                // DCX rp / DCX SP
                let rp = (op >> 4) & 0x3;
                match rp {
                    0 => {
                        let v = (((regs[R_B] as u16) << 8) | (regs[R_C] as u16)).wrapping_sub(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_B, old_value: regs[R_B], new_value: (v >> 8) as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_C, old_value: regs[R_C], new_value: (v & 0xFF) as u32 });
                        regs[R_B] = (v >> 8) as u32; regs[R_C] = (v & 0xFF) as u32;
                    }
                    1 => {
                        let v = (((regs[R_D] as u16) << 8) | (regs[R_E] as u16)).wrapping_sub(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_D, old_value: regs[R_D], new_value: (v >> 8) as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_E, old_value: regs[R_E], new_value: (v & 0xFF) as u32 });
                        regs[R_D] = (v >> 8) as u32; regs[R_E] = (v & 0xFF) as u32;
                    }
                    2 => {
                        let v = get_hl(&regs).wrapping_sub(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: (v >> 8) as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: (v & 0xFF) as u32 });
                        set_hl(&mut regs, v);
                    }
                    _ => {
                        let v = (get_sp(&regs) as u16).wrapping_sub(1);
                        undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: v as u32 });
                        regs[R_SP] = v as u32;
                    }
                }
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch DCX", "Decode", "rp ← rp-1"), 1)
            }
            0x09 | 0x19 | 0x29 | 0x39 => {
                // DAD rp: HL = HL + rp (C set, others unaffected)
                let hl = get_hl(&regs);
                let rp = (op >> 4) & 0x3;
                let rhs: u16 = match rp {
                    0 => ((regs[R_B] as u16) << 8) | (regs[R_C] as u16),
                    1 => ((regs[R_D] as u16) << 8) | (regs[R_E] as u16),
                    2 => hl,
                    _ => get_sp(&regs) as u16,
                };
                let sum = (hl as u32) + (rhs as u32);
                let res = (sum & 0xFFFF) as u16;
                let mut f = regs[R_FLAGS];
                if (sum & 0x1_0000) != 0 { f |= FLAG_C; } else { f &= !FLAG_C; }
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: (res >> 8) as u32 });
                undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: (res & 0xFF) as u32 });
                set_hl(&mut regs, res);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch DAD", "Decode", "HL ← HL + rp"), 1)
            }
            0x2A => {
                // LHLD addr: L <- (addr), H <- (addr+1)
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let addr = lo | (hi << 8);
                let l = mem.read_u8(addr).unwrap_or(0);
                let h = mem.read_u8(addr + 1).unwrap_or(0);
                undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: l as u32 });
                undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: h as u32 });
                regs[R_L] = l as u32; regs[R_H] = h as u32;
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 3, pipeline_3(Some(instr_bits), "Fetch LHLD", "Decode", "HL ← Mem[addr..addr+1]"), 3)
            }
            0x22 => {
                // SHLD addr: (addr) <- L, (addr+1) <- H
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let addr = lo | (hi << 8);
                let l = regs[R_L] as u8;
                let h = regs[R_H] as u8;
                let old0 = mem.read_u8(addr).unwrap_or(0);
                let old1 = mem.read_u8(addr + 1).unwrap_or(0);
                mem.write_u8(addr, l).ok();
                mem.write_u8(addr + 1, h).ok();
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old0, new_value: l });
                undo_log.push(UndoEntry::MemWrite { addr: addr + 1, old_value: old1, new_value: h });
                events.push(TraceEvent::Mem);
                (pc + 3, pipeline_3(Some(instr_bits), "Fetch SHLD", "Decode", "Mem[addr..] ← HL"), 3)
            }
            0xEB => {
                // XCHG: swap DE and HL
                undo_log.push(UndoEntry::RegWrite { reg: R_D, old_value: regs[R_D], new_value: regs[R_H] });
                undo_log.push(UndoEntry::RegWrite { reg: R_E, old_value: regs[R_E], new_value: regs[R_L] });
                undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: regs[R_D] });
                undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: regs[R_E] });
                let d = regs[R_D]; let e = regs[R_E];
                regs[R_D] = regs[R_H]; regs[R_E] = regs[R_L];
                regs[R_H] = d; regs[R_L] = e;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch XCHG", "Decode", "Swap DE <-> HL"), 1)
            }
            0xE3 => {
                // XTHL: exchange HL with top of stack
                let sp = get_sp(&regs);
                let lo = mem.read_u8(sp).unwrap_or(0);
                let hi = mem.read_u8(sp + 1).unwrap_or(0);
                let old_lo = lo;
                let old_hi = hi;
                let l = regs[R_L] as u8;
                let h = regs[R_H] as u8;
                // write old HL into stack
                mem.write_u8(sp, l).ok();
                mem.write_u8(sp + 1, h).ok();
                undo_log.push(UndoEntry::MemWrite { addr: sp, old_value: old_lo, new_value: l });
                undo_log.push(UndoEntry::MemWrite { addr: sp + 1, old_value: old_hi, new_value: h });
                // load stack into HL
                undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: lo as u32 });
                undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: hi as u32 });
                regs[R_L] = lo as u32;
                regs[R_H] = hi as u32;
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch XTHL", "Decode", "Exchange HL with (SP)"), 1)
            }
            0xF9 => {
                // SPHL: SP <- HL
                let hl = get_hl(&regs);
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: hl as u32 });
                regs[R_SP] = hl as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch SPHL", "Decode", "SP ← HL"), 1)
            }
            0xE9 => {
                // PCHL: PC <- HL
                let hl = get_hl(&regs) as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: hl });
                (hl, pipeline_3(Some(instr_bits), "Fetch PCHL", "Decode", "PC ← HL"), 1)
            }
            0x27 => {
                // DAA: decimal adjust accumulator
                let a = regs[R_A] as u8;
                let mut adj: u8 = 0;
                let mut f = regs[R_FLAGS];
                let mut carry = (f & FLAG_C) != 0;
                if (a & 0x0F) > 9 || (f & FLAG_AC) != 0 {
                    adj |= 0x06;
                }
                if (a > 0x99) || carry {
                    adj |= 0x60;
                    carry = true;
                }
                let result = a.wrapping_add(adj);
                f &= !(FLAG_S | FLAG_Z | FLAG_P | FLAG_AC);
                set_szp(&mut f, result);
                if carry { f |= FLAG_C; } else { f &= !FLAG_C; }
                // AC undefined-ish; we approximate from low nibble carry
                if ((a & 0x0F) + (adj & 0x0F)) > 0x0F { f |= FLAG_AC; }
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch DAA", "Decode", "Decimal adjust A"), 1)
            }
            0x2F => {
                // CMA
                let a = regs[R_A] as u8;
                let result = !a;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch CMA", "Decode", "A ← ~A"), 1)
            }
            0x3F => {
                // CMC: complement carry
                let mut f = regs[R_FLAGS];
                if (f & FLAG_C) != 0 { f &= !FLAG_C; } else { f |= FLAG_C; }
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch CMC", "Decode", "C ← ~C"), 1)
            }
            0x37 => {
                // STC
                let f = regs[R_FLAGS] | FLAG_C;
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch STC", "Decode", "C ← 1"), 1)
            }
            0x07 | 0x0F | 0x17 | 0x1F => {
                // RLC / RRC / RAL / RAR
                let a = regs[R_A] as u8;
                let mut f = regs[R_FLAGS] & !(FLAG_C);
                let (result, c) = match op {
                    0x07 => {
                        let c = (a >> 7) & 1;
                        ((a << 1) | c, c)
                    }
                    0x0F => {
                        let c = a & 1;
                        (((c << 7) | (a >> 1)) as u8, c)
                    }
                    0x17 => {
                        let cin = if (regs[R_FLAGS] & FLAG_C) != 0 { 1 } else { 0 };
                        let c = (a >> 7) & 1;
                        (((a << 1) | cin) as u8, c)
                    }
                    _ => {
                        let cin = if (regs[R_FLAGS] & FLAG_C) != 0 { 1 } else { 0 };
                        let c = a & 1;
                        (((a >> 1) | (cin << 7)) as u8, c)
                    }
                };
                if c != 0 { f |= FLAG_C; }
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                regs[R_FLAGS] = f;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: result as u32 });
                regs[R_A] = result as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch ROT", "Decode", "Rotate A"), 1)
            }
            0xDB => {
                // IN port: for now, return 0 (no hardware)
                let _port = mem.read_u8(pc + 1).unwrap_or(0);
                let val: u8 = 0;
                undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: val as u32 });
                regs[R_A] = val as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch IN", "Decode", "A ← IN(port)=0"), 2)
            }
            0xD3 => {
                // OUT port: ignore
                let _port = mem.read_u8(pc + 1).unwrap_or(0);
                (pc + 2, pipeline_3(Some(instr_bits), "Fetch OUT", "Decode", "OUT ignored"), 2)
            }
            0xF3 | 0xFB | 0x20 | 0x30 => {
                // DI/EI, RIM/SIM (treat SIM/RIM as NOP; EI/DI are NOP in this single-threaded simulator)
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch", "Decode", "NOP"), 1)
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                // RST n: CALL to vector n*8
                let vec = (op & 0x38) as u32;
                let ret_addr = pc + 1;
                let sp = get_sp(&regs);
                let sp_new = sp.wrapping_sub(2);
                set_sp(&mut regs, sp_new as u16);
                let old_lo = mem.read_u8(sp_new).unwrap_or(0);
                let old_hi = mem.read_u8(sp_new + 1).unwrap_or(0);
                mem.write_u8(sp_new, (ret_addr & 0xFF) as u8).ok();
                mem.write_u8(sp_new + 1, (ret_addr >> 8) as u8).ok();
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
                undo_log.push(UndoEntry::MemWrite { addr: sp_new, old_value: old_lo, new_value: (ret_addr & 0xFF) as u8 });
                undo_log.push(UndoEntry::MemWrite { addr: sp_new + 1, old_value: old_hi, new_value: (ret_addr >> 8) as u8 });
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: vec });
                events.push(TraceEvent::Mem);
                (vec, pipeline_3(Some(instr_bits), "Fetch RST", "Decode", &format!("Call 0x{:02X}", vec)), 1)
            }
            0xC3 => {
                // JMP addr
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                (target, pipeline_3(Some(instr_bits), "Fetch JMP", "Decode", &format!("PC ← 0x{:04X}", target)), 3)
            }
            0xC2 | 0xCA | 0xD2 | 0xDA | 0xE2 | 0xEA | 0xF2 | 0xFA => {
                // Conditional JMP: JNZ/JZ/JNC/JC/JPO/JPE/JP/JM
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                let cond = match op {
                    0xC2 => (flags & FLAG_Z) == 0,
                    0xCA => (flags & FLAG_Z) != 0,
                    0xD2 => (flags & FLAG_C) == 0,
                    0xDA => (flags & FLAG_C) != 0,
                    0xE2 => (flags & FLAG_P) == 0,
                    0xEA => (flags & FLAG_P) != 0,
                    0xF2 => (flags & FLAG_S) == 0,
                    _ => (flags & FLAG_S) != 0,
                };
                let next = if cond { target } else { pc + 3 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                (next, pipeline_3(Some(instr_bits), "Fetch Jcc", "Decode", if cond { "Taken" } else { "Not taken" }), 3)
            }
            0xCD => {
                // CALL addr: push PC+3, jump
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                let ret_addr = pc + 3;
                let sp = get_sp(&regs);
                let sp_new = sp.wrapping_sub(2);
                set_sp(&mut regs, sp_new as u16);
                let old_lo = mem.read_u8(sp_new).unwrap_or(0);
                let old_hi = mem.read_u8(sp_new + 1).unwrap_or(0);
                mem.write_u8(sp_new, (ret_addr & 0xFF) as u8).ok();
                mem.write_u8(sp_new + 1, (ret_addr >> 8) as u8).ok();
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
                undo_log.push(UndoEntry::MemWrite { addr: sp_new, old_value: old_lo, new_value: (ret_addr & 0xFF) as u8 });
                undo_log.push(UndoEntry::MemWrite { addr: sp_new + 1, old_value: old_hi, new_value: (ret_addr >> 8) as u8 });
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                events.push(TraceEvent::Mem);
                (target, pipeline_3(Some(instr_bits), "Fetch CALL", "Decode", &format!("Push 0x{:04X}, PC ← 0x{:04X}", ret_addr, target)), 3)
            }
            0xC4 | 0xCC | 0xD4 | 0xDC | 0xE4 | 0xEC | 0xF4 | 0xFC => {
                // Conditional CALL: CNZ/CZ/CNC/CC/CPO/CPE/CP/CM
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                let cond = match op {
                    0xC4 => (flags & FLAG_Z) == 0,
                    0xCC => (flags & FLAG_Z) != 0,
                    0xD4 => (flags & FLAG_C) == 0,
                    0xDC => (flags & FLAG_C) != 0,
                    0xE4 => (flags & FLAG_P) == 0,
                    0xEC => (flags & FLAG_P) != 0,
                    0xF4 => (flags & FLAG_S) == 0,
                    _ => (flags & FLAG_S) != 0,
                };
                if cond {
                    let ret_addr = pc + 3;
                    let sp = get_sp(&regs);
                    let sp_new = sp.wrapping_sub(2);
                    set_sp(&mut regs, sp_new as u16);
                    let old_lo = mem.read_u8(sp_new).unwrap_or(0);
                    let old_hi = mem.read_u8(sp_new + 1).unwrap_or(0);
                    mem.write_u8(sp_new, (ret_addr & 0xFF) as u8).ok();
                    mem.write_u8(sp_new + 1, (ret_addr >> 8) as u8).ok();
                    undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
                    undo_log.push(UndoEntry::MemWrite { addr: sp_new, old_value: old_lo, new_value: (ret_addr & 0xFF) as u8 });
                    undo_log.push(UndoEntry::MemWrite { addr: sp_new + 1, old_value: old_hi, new_value: (ret_addr >> 8) as u8 });
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                    events.push(TraceEvent::Mem);
                    (target, pipeline_3(Some(instr_bits), "Fetch Ccc", "Decode", "Taken"), 3)
                } else {
                    let next = pc + 3;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                    (next, pipeline_3(Some(instr_bits), "Fetch Ccc", "Decode", "Not taken"), 3)
                }
            }
            0xC9 => {
                // RET: pop PC
                let sp = get_sp(&regs);
                let lo = mem.read_u8(sp).unwrap_or(0) as u32;
                let hi = mem.read_u8(sp + 1).unwrap_or(0) as u32;
                let target = lo | (hi << 8);
                set_sp(&mut regs, (sp + 2) as u16);
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: (sp + 2) as u32 });
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                events.push(TraceEvent::Mem);
                (target, pipeline_3(Some(instr_bits), "Fetch RET", "Decode", &format!("PC ← 0x{:04X}", target)), 1)
            }
            0xC0 | 0xC8 | 0xD0 | 0xD8 | 0xE0 | 0xE8 | 0xF0 | 0xF8 => {
                // RNZ / RZ / RNC / RC / RPO / RPE / RP / RM
                let cond = match op {
                    0xC0 => (flags & FLAG_Z) == 0,
                    0xC8 => (flags & FLAG_Z) != 0,
                    0xD0 => (flags & FLAG_C) == 0,
                    0xD8 => (flags & FLAG_C) != 0,
                    0xE0 => (flags & FLAG_P) == 0,
                    0xE8 => (flags & FLAG_P) != 0,
                    0xF0 => (flags & FLAG_S) == 0,
                    _ => (flags & FLAG_S) != 0,
                };
                if cond {
                    let sp = get_sp(&regs);
                    let lo = mem.read_u8(sp).unwrap_or(0) as u32;
                    let hi = mem.read_u8(sp + 1).unwrap_or(0) as u32;
                    let target = lo | (hi << 8);
                    set_sp(&mut regs, (sp + 2) as u16);
                    undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: (sp + 2) as u32 });
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target });
                    events.push(TraceEvent::Mem);
                    (target, pipeline_3(Some(instr_bits), "Fetch RETcc", "Decode", "Taken"), 1)
                } else {
                    let next = pc + 1;
                    undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next });
                    (next, pipeline_3(Some(instr_bits), "Fetch RETcc", "Decode", "Not taken"), 1)
                }
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                // PUSH rp (B/D/H/PSW)
                let sp = get_sp(&regs);
                let sp_new = sp.wrapping_sub(2);
                set_sp(&mut regs, sp_new as u16);
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });

                let (hi, lo) = match op {
                    0xC5 => (regs[R_B] as u8, regs[R_C] as u8),
                    0xD5 => (regs[R_D] as u8, regs[R_E] as u8),
                    0xE5 => (regs[R_H] as u8, regs[R_L] as u8),
                    _ => {
                        // PSW: A (hi) and flags (lo) packed similar to 8080/8085
                        let a = regs[R_A] as u8;
                        let f = regs[R_FLAGS] as u8;
                        let packed =
                            (if (f & (FLAG_S as u8)) != 0 { 0x80 } else { 0 }) |
                            (if (f & (FLAG_Z as u8)) != 0 { 0x40 } else { 0 }) |
                            (if (f & (FLAG_AC as u8)) != 0 { 0x10 } else { 0 }) |
                            (if (f & (FLAG_P as u8)) != 0 { 0x04 } else { 0 }) |
                            0x02 | // bit1 is always 1
                            (if (f & (FLAG_C as u8)) != 0 { 0x01 } else { 0 });
                        (a, packed)
                    }
                };

                let old0 = mem.read_u8(sp_new).unwrap_or(0);
                let old1 = mem.read_u8(sp_new + 1).unwrap_or(0);
                // 8080/8085 push stores low at [SP], high at [SP+1] after decrementing
                mem.write_u8(sp_new, lo).ok();
                mem.write_u8(sp_new + 1, hi).ok();
                undo_log.push(UndoEntry::MemWrite { addr: sp_new, old_value: old0, new_value: lo });
                undo_log.push(UndoEntry::MemWrite { addr: sp_new + 1, old_value: old1, new_value: hi });
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: pc + 1 });
                events.push(TraceEvent::Mem);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch PUSH", "Decode", "Push rp"), 1)
            }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                // POP rp (B/D/H/PSW)
                let sp = get_sp(&regs);
                let lo = mem.read_u8(sp).unwrap_or(0);
                let hi = mem.read_u8(sp + 1).unwrap_or(0);
                set_sp(&mut regs, (sp + 2) as u16);
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: (sp + 2) as u32 });

                match op {
                    0xC1 => {
                        undo_log.push(UndoEntry::RegWrite { reg: R_C, old_value: regs[R_C], new_value: lo as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_B, old_value: regs[R_B], new_value: hi as u32 });
                        regs[R_C] = lo as u32;
                        regs[R_B] = hi as u32;
                    }
                    0xD1 => {
                        undo_log.push(UndoEntry::RegWrite { reg: R_E, old_value: regs[R_E], new_value: lo as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_D, old_value: regs[R_D], new_value: hi as u32 });
                        regs[R_E] = lo as u32;
                        regs[R_D] = hi as u32;
                    }
                    0xE1 => {
                        undo_log.push(UndoEntry::RegWrite { reg: R_L, old_value: regs[R_L], new_value: lo as u32 });
                        undo_log.push(UndoEntry::RegWrite { reg: R_H, old_value: regs[R_H], new_value: hi as u32 });
                        regs[R_L] = lo as u32;
                        regs[R_H] = hi as u32;
                    }
                    _ => {
                        // PSW: A = hi, flags = lo
                        undo_log.push(UndoEntry::RegWrite { reg: R_A, old_value: regs[R_A], new_value: hi as u32 });
                        regs[R_A] = hi as u32;
                        // unpack flags into our SZACP bitset
                        let mut f: u32 = 0;
                        if (lo & 0x80) != 0 { f |= FLAG_S; }
                        if (lo & 0x40) != 0 { f |= FLAG_Z; }
                        if (lo & 0x10) != 0 { f |= FLAG_AC; }
                        if (lo & 0x04) != 0 { f |= FLAG_P; }
                        if (lo & 0x01) != 0 { f |= FLAG_C; }
                        undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: f });
                        regs[R_FLAGS] = f;
                    }
                }

                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: pc + 1 });
                events.push(TraceEvent::Mem);
                events.push(TraceEvent::RegWrite);
                (pc + 1, pipeline_3(Some(instr_bits), "Fetch POP", "Decode", "Pop rp"), 1)
            }
            0x01 | 0x11 | 0x21 => {
                // LXI rp, d16  (BC=0x01, DE=0x11, HL=0x21)
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u16;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u16;
                let val = lo | (hi << 8);
                let (r1, r2) = match op {
                    0x01 => (R_B, R_C),
                    0x11 => (R_D, R_E),
                    _ => (R_H, R_L),
                };
                undo_log.push(UndoEntry::RegWrite { reg: r1, old_value: regs[r1], new_value: (val >> 8) as u32 });
                undo_log.push(UndoEntry::RegWrite { reg: r2, old_value: regs[r2], new_value: (val & 0xFF) as u32 });
                regs[r1] = (val >> 8) as u32;
                regs[r2] = (val & 0xFF) as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 3, pipeline_3(Some(instr_bits), "Fetch LXI", "Decode", &format!("rp ← 0x{:04X}", val)), 3)
            }
            0x31 => {
                // LXI SP, d16
                let lo = mem.read_u8(pc + 1).unwrap_or(0) as u16;
                let hi = mem.read_u8(pc + 2).unwrap_or(0) as u16;
                let val = lo | (hi << 8);
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: val as u32 });
                regs[R_SP] = val as u32;
                events.push(TraceEvent::RegWrite);
                (pc + 3, pipeline_3(Some(instr_bits), "Fetch LXI SP", "Decode", &format!("SP ← 0x{:04X}", val)), 3)
            }
            _ => {
                return StepResult {
                    new_state: state.clone(),
                    events: vec![],
                    undo_log: vec![],
                    cycles_added: 0,
                    halted: false,
                    error: Some(format!("Unknown 8085 opcode 0x{:02X} at PC=0x{:04X}", op, pc)),
                    instruction_bits: Some(instr_bits),
                    pipeline_stages: vec![],
                    io_output: None,
                    io_input_requested: None,
                };
            }
        };

        let halted = op == 0x76;
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
                UiBlock { id: "regfile".into(), label: "A,B,C,D,E,H,L".into(), x: 185.0, y: 70.0, width: 85.0, height: 45.0 },
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
            reg_names: vec![
                "A".into(), "B".into(), "C".into(), "D".into(), "E".into(), "H".into(), "L".into(),
                "SP".into(), "FLAGS (SZACP)".into(),
            ],
        }
    }
}

fn parse_8085_instruction(
    line: &str,
    line_num: u32,
    col: u32,
    pc: u32,
    labels: &HashMap<String, u32>,
    bytes: &mut Vec<u8>,
    source_map: &mut Vec<SourceMapEntry>,
    _errors: &mut Vec<AssemblerError>,
    pending_refs: &mut Vec<(usize, String, bool)>,
) -> Result<(), AssemblerError> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }
    let mnemonic = parts[0].to_uppercase();
    let args_str: String = if parts.len() > 1 { parts[1..].join(" ") } else { String::new() };
    let args: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();

    let _start = bytes.len();
    source_map.push(SourceMapEntry { pc, line: line_num, column: col });

    match mnemonic.as_str() {
        "NOP" => bytes.push(0x00),
        "HLT" => bytes.push(0x76),
        "EI" => bytes.push(0xFB),
        "DI" => bytes.push(0xF3),
        "RIM" => bytes.push(0x20),
        "SIM" => bytes.push(0x30),
        "MOV" => {
            if args.len() >= 2 {
                let dst = parse_reg_8085(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid MOV destination".into() })?;
                let src = parse_reg_8085(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid MOV source".into() })?;
                bytes.push(0x40 | ((dst as u8) << 3) | (src as u8));
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "MOV needs 2 operands".into() });
            }
        }
        "MVI" => {
            if args.len() >= 2 {
                let r = parse_reg_8085(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid MVI register".into() })?;
                let imm = parse_imm8(args[1]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid immediate: {}", args[1]) })?;
                bytes.push(0x06 | ((r as u8) << 3));
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "MVI needs register, immediate".into() });
            }
        }
        "INR" | "DCR" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: format!("{} r", mnemonic) });
            }
            let r = parse_reg_8085(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid register".into() })? as u8;
            let base = if mnemonic == "INR" { 0x04 } else { 0x05 };
            bytes.push(base | (r << 3));
        }
        "INX" | "DCX" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: format!("{} rp", mnemonic) });
            }
            let rp = args[0].to_uppercase();
            let code = match rp.as_str() {
                "B" | "BC" => 0,
                "D" | "DE" => 1,
                "H" | "HL" => 2,
                "SP" => 3,
                _ => return Err(AssemblerError { line: line_num, column: col, message: "Invalid register pair".into() }),
            };
            let base = if mnemonic == "INX" { 0x03 } else { 0x0B };
            bytes.push(base | ((code as u8) << 4));
        }
        "DAD" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "DAD rp".into() });
            }
            let rp = args[0].to_uppercase();
            let code = match rp.as_str() {
                "B" | "BC" => 0,
                "D" | "DE" => 1,
                "H" | "HL" => 2,
                "SP" => 3,
                _ => return Err(AssemblerError { line: line_num, column: col, message: "Invalid register pair".into() }),
            };
            bytes.push(0x09 | ((code as u8) << 4));
        }
        "LDA" => {
            if args.len() >= 1 {
                bytes.push(0x3A);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "LDA needs address".into() });
            }
        }
        "LDAX" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "LDAX B|D".into() });
            }
            let rp = args[0].to_uppercase();
            let op = match rp.as_str() {
                "B" | "BC" => 0x0A,
                "D" | "DE" => 0x1A,
                _ => return Err(AssemblerError { line: line_num, column: col, message: "LDAX expects BC or DE".into() }),
            };
            bytes.push(op);
        }
        "STA" => {
            if args.len() >= 1 {
                bytes.push(0x32);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "STA needs address".into() });
            }
        }
        "STAX" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "STAX B|D".into() });
            }
            let rp = args[0].to_uppercase();
            let op = match rp.as_str() {
                "B" | "BC" => 0x02,
                "D" | "DE" => 0x12,
                _ => return Err(AssemblerError { line: line_num, column: col, message: "STAX expects BC or DE".into() }),
            };
            bytes.push(op);
        }
        "LHLD" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "LHLD addr".into() });
            }
            bytes.push(0x2A);
            let off = bytes.len();
            bytes.push(0);
            bytes.push(0);
            resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
        }
        "SHLD" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "SHLD addr".into() });
            }
            bytes.push(0x22);
            let off = bytes.len();
            bytes.push(0);
            bytes.push(0);
            resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
        }
        "ADD" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0x80 | r as u8);
        }
        "ADC" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0x88 | r as u8);
        }
        "ADI" => {
            if args.len() >= 1 {
                let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid immediate: {}", args[0]) })?;
                bytes.push(0xC6);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "ADI needs immediate".into() });
            }
        }
        "ACI" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "ACI d8".into() });
            }
            let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid imm".into() })?;
            bytes.push(0xCE);
            bytes.push(imm);
        }
        "SUB" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0x90 | r as u8);
        }
        "SBB" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0x98 | r as u8);
        }
        "SUI" => {
            if args.len() >= 1 {
                let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: format!("Invalid immediate: {}", args[0]) })?;
                bytes.push(0xD6);
                bytes.push(imm);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "SUI needs immediate".into() });
            }
        }
        "SBI" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "SBI d8".into() });
            }
            let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid imm".into() })?;
            bytes.push(0xDE);
            bytes.push(imm);
        }
        "ANA" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0xA0 | r as u8);
        }
        "XRA" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0xA8 | r as u8);
        }
        "ORA" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0xB0 | r as u8);
        }
        "CMP" => {
            let r = if args.is_empty() { 6 } else { parse_reg_8085(args[0]).unwrap_or(6) };
            bytes.push(0xB8 | r as u8);
        }
        "ANI" | "XRI" | "ORI" | "CPI" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: format!("{} d8", mnemonic) });
            }
            let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid imm".into() })?;
            let op = match mnemonic.as_str() {
                "ANI" => 0xE6,
                "XRI" => 0xEE,
                "ORI" => 0xF6,
                _ => 0xFE,
            };
            bytes.push(op);
            bytes.push(imm);
        }
        "DAA" => bytes.push(0x27),
        "CMA" => bytes.push(0x2F),
        "CMC" => bytes.push(0x3F),
        "STC" => bytes.push(0x37),
        "RLC" => bytes.push(0x07),
        "RRC" => bytes.push(0x0F),
        "RAL" => bytes.push(0x17),
        "RAR" => bytes.push(0x1F),
        "XCHG" => bytes.push(0xEB),
        "XTHL" => bytes.push(0xE3),
        "SPHL" => bytes.push(0xF9),
        "PCHL" => bytes.push(0xE9),
        "IN" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "IN port".into() });
            }
            let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid port".into() })?;
            bytes.push(0xDB);
            bytes.push(imm);
        }
        "OUT" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "OUT port".into() });
            }
            let imm = parse_imm8(args[0]).map_err(|_| AssemblerError { line: line_num, column: col, message: "Invalid port".into() })?;
            bytes.push(0xD3);
            bytes.push(imm);
        }
        "RST" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "RST n (0-7)".into() });
            }
            let n: u8 = args[0].parse().map_err(|_| AssemblerError { line: line_num, column: col, message: "RST n must be decimal 0-7".into() })?;
            if n > 7 {
                return Err(AssemblerError { line: line_num, column: col, message: "RST n must be 0-7".into() });
            }
            bytes.push(0xC7 | (n << 3));
        }
        "JMP" => {
            if args.len() >= 1 {
                bytes.push(0xC3);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "JMP needs address".into() });
            }
        }
        "JPO" | "JPE" | "JP" | "JM" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "Jcc addr".into() });
            }
            let op = match mnemonic.as_str() {
                "JPO" => 0xE2,
                "JPE" => 0xEA,
                "JP" => 0xF2,
                _ => 0xFA,
            };
            bytes.push(op);
            let off = bytes.len();
            bytes.push(0);
            bytes.push(0);
            resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
        }
        "JZ" | "JNZ" | "JC" | "JNC" => {
            if args.len() >= 1 {
                let op = match mnemonic.as_str() {
                    "JZ" => 0xCA,
                    "JNZ" => 0xC2,
                    "JC" => 0xDA,
                    _ => 0xD2,
                };
                bytes.push(op);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "Jump needs address".into() });
            }
        }
        "CALL" => {
            if args.len() >= 1 {
                bytes.push(0xCD);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "CALL needs address".into() });
            }
        }
        "CNZ" | "CZ" | "CNC" | "CC" | "CPO" | "CPE" | "CP" | "CM" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "Ccc addr".into() });
            }
            let op = match mnemonic.as_str() {
                "CNZ" => 0xC4,
                "CZ" => 0xCC,
                "CNC" => 0xD4,
                "CC" => 0xDC,
                "CPO" => 0xE4,
                "CPE" => 0xEC,
                "CP" => 0xF4,
                _ => 0xFC,
            };
            bytes.push(op);
            let off = bytes.len();
            bytes.push(0);
            bytes.push(0);
            resolve_addr(args[0], labels, pending_refs, off, line_num, true)?;
        }
        "RET" => bytes.push(0xC9),
        "RNZ" => bytes.push(0xC0),
        "RZ" => bytes.push(0xC8),
        "RNC" => bytes.push(0xD0),
        "RC" => bytes.push(0xD8),
        "RPO" => bytes.push(0xE0),
        "RPE" => bytes.push(0xE8),
        "RP" => bytes.push(0xF0),
        "RM" => bytes.push(0xF8),
        "PUSH" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "PUSH rp (B/D/H/PSW)".into() });
            }
            let rp = args[0].to_uppercase();
            let op = match rp.as_str() {
                "B" | "BC" => 0xC5,
                "D" | "DE" => 0xD5,
                "H" | "HL" => 0xE5,
                "PSW" => 0xF5,
                _ => return Err(AssemblerError { line: line_num, column: col, message: "PUSH expects BC, DE, HL, or PSW".into() }),
            };
            bytes.push(op);
        }
        "POP" => {
            if args.len() != 1 {
                return Err(AssemblerError { line: line_num, column: col, message: "POP rp (B/D/H/PSW)".into() });
            }
            let rp = args[0].to_uppercase();
            let op = match rp.as_str() {
                "B" | "BC" => 0xC1,
                "D" | "DE" => 0xD1,
                "H" | "HL" => 0xE1,
                "PSW" => 0xF1,
                _ => return Err(AssemblerError { line: line_num, column: col, message: "POP expects BC, DE, HL, or PSW".into() }),
            };
            bytes.push(op);
        }
        "LXI" => {
            if args.len() >= 2 {
                let rp = args[0].to_uppercase();
                let op = match rp.as_str() {
                    "B" | "BC" => 0x01,
                    "D" | "DE" => 0x11,
                    "H" | "HL" => 0x21,
                    "SP" => 0x31,
                    _ => return Err(AssemblerError { line: line_num, column: col, message: "LXI: expect BC, DE, HL, or SP".into() }),
                };
                bytes.push(op);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                match parse_imm16(args[1]) {
                    Ok(val) => {
                        bytes[off] = (val & 0xFF) as u8;
                        bytes[off + 1] = (val >> 8) as u8;
                    }
                    Err(()) => {
                        pending_refs.push((off, args[1].to_string(), true));
                    }
                }
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "LXI needs rp, data16".into() });
            }
        }
        _ => return Err(AssemblerError { line: line_num, column: col, message: format!("Unknown mnemonic: {}", mnemonic) }),
    }
    Ok(())
}

fn resolve_addr(
    s: &str,
    labels: &HashMap<String, u32>,
    pending_refs: &mut Vec<(usize, String, bool)>,
    offset: usize,
    _line: u32,
    is_16bit: bool,
) -> Result<u32, AssemblerError> {
    if let Some(&v) = labels.get(s) {
        return Ok(v);
    }
    if let Ok(v) = parse_imm16(s) {
        return Ok(v as u32);
    }
    pending_refs.push((offset, s.to_string(), is_16bit));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i8085_inr_dcr_preserve_carry_and_roundtrip_value() {
        let p = I8085Plugin::new();
        let src = r#"
.ORG 0
_start:
  MVI A, 0x0F
  STC
  INR A
  DCR A
  HLT
"#;
        let img = p.assemble(src);
        assert!(img.errors.is_empty(), "errors: {:?}", img.errors);
        let mut mem = vec![0u8; 0x10000];
        mem[0..img.bytes.len()].copy_from_slice(&img.bytes);
        let mut state = p.reset(&ResetConfig::default());
        state.pc = img.entry_pc;
        // MVI
        state = p.step(&state, &mem, StepMode::Instruction, None).new_state;
        // STC
        state = p.step(&state, &mem, StepMode::Instruction, None).new_state;
        let carry_before = state.regs[R_FLAGS] & FLAG_C;
        // INR
        state = p.step(&state, &mem, StepMode::Instruction, None).new_state;
        assert_eq!(carry_before, state.regs[R_FLAGS] & FLAG_C);
        // DCR
        state = p.step(&state, &mem, StepMode::Instruction, None).new_state;
        assert_eq!(carry_before, state.regs[R_FLAGS] & FLAG_C);
        assert_eq!(state.regs[R_A] as u8, 0x0F);
    }
}
