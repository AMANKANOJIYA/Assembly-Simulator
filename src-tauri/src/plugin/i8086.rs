//! Intel 8086 16-bit CPU: minimal assembler and executor.
//! Registers: AX, BX, CX, DX, SI, DI, BP, SP, FLAGS, CS, DS, SS, ES.
//! Uses flat model (linear address = segment*16 + offset). Subset: MOV, ADD, SUB, PUSH, POP, JMP, JZ, JNZ, CALL, RET, INT, HLT.

use crate::memory::Memory;
use crate::plugin::*;
use std::collections::HashMap;

pub struct I8086Plugin;

impl I8086Plugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for I8086Plugin {
    fn default() -> Self {
        Self::new()
    }
}

// regs: AX, BX, CX, DX, SI, DI, BP, SP (16-bit each), FLAGS, CS, DS, SS, ES
const R_AX: usize = 0;
const R_BX: usize = 1;
const R_CX: usize = 2;
const R_DX: usize = 3;
const R_SI: usize = 4;
const R_DI: usize = 5;
const R_BP: usize = 6;
const R_SP: usize = 7;
const R_FLAGS: usize = 8;
const R_CS: usize = 9;
const R_DS: usize = 10;
const R_SS: usize = 11;
const R_ES: usize = 12;

const FLAG_Z: u32 = 0x40;
const FLAG_C: u32 = 1;

fn linear_addr(seg: u16, offset: u16) -> u32 {
    ((seg as u32) << 4) + (offset as u32)
}

fn pipeline_3(instr: Option<u32>, fetch: &str, decode: &str, execute: &str) -> Vec<PipelineCycleInfo> {
    vec![
        PipelineCycleInfo { stage: "Fetch".into(), instruction_bits: instr, action: fetch.into() },
        PipelineCycleInfo { stage: "Decode".into(), instruction_bits: instr, action: decode.into() },
        PipelineCycleInfo { stage: "Execute".into(), instruction_bits: instr, action: execute.into() },
    ]
}

fn parse_reg_8086(s: &str) -> Option<usize> {
    let s = s.trim().to_uppercase();
    match s.as_str() {
        "AX" => Some(R_AX),
        "BX" => Some(R_BX),
        "CX" => Some(R_CX),
        "DX" => Some(R_DX),
        "SI" => Some(R_SI),
        "DI" => Some(R_DI),
        "BP" => Some(R_BP),
        "SP" => Some(R_SP),
        _ => None,
    }
}

fn parse_imm16(s: &str) -> Result<u16, ()> {
    let s = s.trim();
    let s = s.trim_start_matches("0x").trim_start_matches('h').trim_end_matches('h');
    u16::from_str_radix(s, 16).or_else(|_| s.parse::<u16>().map_err(|_| ()))
}

impl ArchitecturePlugin for I8086Plugin {
    fn name(&self) -> &str {
        "8086"
    }

    fn assemble(&self, source: &str) -> ProgramImage {
        let mut bytes = Vec::new();
        let mut source_map = Vec::new();
        let mut errors = Vec::new();
        let mut labels: HashMap<String, u32> = HashMap::new();
        let mut pending_refs: Vec<(usize, String, bool)> = Vec::new(); // (offset, label, is_16bit)

        let lines: Vec<&str> = source.lines().collect();
        let mut pc: u32 = 0;

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
                if let Err(e) = parse_8086_instruction(
                    rest, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                    &mut errors, &mut pending_refs,
                ) {
                    errors.push(e);
                }
                pc = pc.wrapping_add((bytes.len() - start_len) as u32);
                continue;
            }

            if let Err(e) = parse_8086_instruction(
                line, line_num, col, pc, &labels, &mut bytes, &mut source_map,
                &mut errors, &mut pending_refs,
            ) {
                errors.push(e);
            }
            pc = pc.wrapping_add((bytes.len() - start_len) as u32);
        }

        for (offset, label, is_16bit) in pending_refs {
            if let Some(&target) = labels.get(&label) {
                if is_16bit && offset + 1 < bytes.len() {
                    let rel = (target as i32) - (offset as i32) - 3;
                    bytes[offset] = (rel & 0xFF) as u8;
                    bytes[offset + 1] = ((rel >> 8) & 0xFF) as u8;
                } else if !is_16bit && offset < bytes.len() {
                    let rel = (target as i32) - (offset as i32) - 2;
                    if rel >= -128 && rel <= 127 {
                        bytes[offset] = rel as u8;
                    }
                }
            } else {
                errors.push(AssemblerError { line: 0, column: 1, message: format!("Unknown label: {}", label) });
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
            regs: vec![0u32; 13],
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

        let pc = state.pc;
        let mut regs = state.regs.clone();
        let ss = (regs[R_SS] & 0xFFFF) as u16;
        let linear = pc;

        let op = match mem.read_u8(linear) {
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

        let get_sp = |r: &[u32]| (r[R_SP] & 0xFFFF) as u16;
        let set_sp = |r: &mut [u32], v: u16| r[R_SP] = v as u32;

        let (next_linear, pipeline_stages, cycles, halted) = match op {
            0x90 => {
                // NOP
                (linear + 1, pipeline_3(Some(instr_bits), "Fetch NOP", "Decode", "NOP"), 1, false)
            }
            0xF4 => {
                // HLT
                (
                    linear,
                    pipeline_3(Some(instr_bits), "Fetch HLT", "Decode", "Halt"),
                    1,
                    true,
                )
            }
            0x8B => {
                // MOV reg, reg (0x8B modrm: 11 dest reg, src reg) - 0x8B 0xC0 + (dest<<3) + src
                let modrm = mem.read_u8(linear + 1).unwrap_or(0);
                let dst = ((modrm >> 3) & 7) as usize;
                let src = (modrm & 7) as usize;
                let val = regs[src] & 0xFFFF;
                undo_log.push(UndoEntry::RegWrite { reg: dst, old_value: regs[dst], new_value: val });
                regs[dst] = val;
                events.push(TraceEvent::RegWrite);
                (linear + 2, pipeline_3(Some(instr_bits), "Fetch MOV", "Decode", &format!("reg{} ← reg{}", dst, src)), 2, false)
            }
            0xB8..=0xBF => {
                // MOV reg, imm16 (0xB8 + reg)
                let r = (op & 7) as usize;
                let lo = mem.read_u8(linear + 1).unwrap_or(0) as u32;
                let hi = mem.read_u8(linear + 2).unwrap_or(0) as u32;
                let val = lo | (hi << 8);
                undo_log.push(UndoEntry::RegWrite { reg: r, old_value: regs[r], new_value: val });
                regs[r] = val;
                events.push(TraceEvent::RegWrite);
                (linear + 3, pipeline_3(Some(instr_bits), "Fetch MOV reg,imm", "Decode", &format!("reg{} ← 0x{:04X}", r, val)), 3, false)
            }
            0x01..=0x03 => {
                // ADD reg, reg (0x01 modrm = add r/m, r; 0x03 modrm = add r, r/m). Use 0x03: add dest, src
                let modrm = mem.read_u8(linear + 1).unwrap_or(0);
                let dst = ((modrm >> 3) & 7) as usize;
                let src = (modrm & 7) as usize;
                let a = (regs[dst] & 0xFFFF) as u16;
                let b = (regs[src] & 0xFFFF) as u16;
                let (result, carry) = a.overflowing_add(b);
                let flags = regs[R_FLAGS] & !(FLAG_Z | FLAG_C);
                let flags = flags | (if result == 0 { FLAG_Z } else { 0 }) | (if carry { FLAG_C } else { 0 });
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: flags });
                regs[R_FLAGS] = flags;
                undo_log.push(UndoEntry::RegWrite { reg: dst, old_value: regs[dst], new_value: result as u32 });
                regs[dst] = result as u32;
                events.push(TraceEvent::RegWrite);
                (linear + 2, pipeline_3(Some(instr_bits), "Fetch ADD", "Decode", &format!("reg{} ← reg{} + reg{}", dst, dst, src)), 2, false)
            }
            0x29..=0x2B => {
                // SUB reg, reg
                let modrm = mem.read_u8(linear + 1).unwrap_or(0);
                let dst = ((modrm >> 3) & 7) as usize;
                let src = (modrm & 7) as usize;
                let a = (regs[dst] & 0xFFFF) as u16;
                let b = (regs[src] & 0xFFFF) as u16;
                let (result, borrow) = a.overflowing_sub(b);
                let flags = regs[R_FLAGS] & !(FLAG_Z | FLAG_C);
                let flags = flags | (if result == 0 { FLAG_Z } else { 0 }) | (if !borrow { FLAG_C } else { 0 });
                undo_log.push(UndoEntry::RegWrite { reg: R_FLAGS, old_value: regs[R_FLAGS], new_value: flags });
                regs[R_FLAGS] = flags;
                undo_log.push(UndoEntry::RegWrite { reg: dst, old_value: regs[dst], new_value: result as u32 });
                regs[dst] = result as u32;
                events.push(TraceEvent::RegWrite);
                (linear + 2, pipeline_3(Some(instr_bits), "Fetch SUB", "Decode", &format!("reg{} ← reg{} - reg{}", dst, dst, src)), 2, false)
            }
            0x50..=0x57 => {
                // PUSH reg (0x50 + reg)
                let r = (op & 7) as usize;
                let sp = get_sp(&regs);
                let sp_new = sp.wrapping_sub(2);
                let addr = linear_addr(ss, sp_new);
                let val = (regs[r] & 0xFFFF) as u16;
                let old_lo = mem.read_u8(addr).unwrap_or(0);
                let old_hi = mem.read_u8(addr + 1).unwrap_or(0);
                mem.write_u8(addr, (val & 0xFF) as u8).ok();
                mem.write_u8(addr + 1, (val >> 8) as u8).ok();
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old_lo, new_value: (val & 0xFF) as u8 });
                undo_log.push(UndoEntry::MemWrite { addr: addr + 1, old_value: old_hi, new_value: (val >> 8) as u8 });
                set_sp(&mut regs, sp_new);
                events.push(TraceEvent::Mem);
                (linear + 1, pipeline_3(Some(instr_bits), "Fetch PUSH", "Decode", &format!("Push reg{}", r)), 1, false)
            }
            0x58..=0x5F => {
                // POP reg (0x58 + reg)
                let r = (op & 7) as usize;
                let sp = get_sp(&regs);
                let addr = linear_addr(ss, sp);
                let lo = mem.read_u8(addr).unwrap_or(0) as u32;
                let hi = mem.read_u8(addr + 1).unwrap_or(0) as u32;
                let val = lo | (hi << 8);
                undo_log.push(UndoEntry::RegWrite { reg: r, old_value: regs[r], new_value: val });
                regs[r] = val;
                set_sp(&mut regs, sp.wrapping_add(2));
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: (sp + 2) as u32 });
                regs[R_SP] = (sp + 2) as u32;
                events.push(TraceEvent::Mem);
                (linear + 1, pipeline_3(Some(instr_bits), "Fetch POP", "Decode", &format!("Pop reg{}", r)), 1, false)
            }
            0xE9 => {
                // JMP rel16 (near)
                let lo = mem.read_u8(linear + 1).unwrap_or(0) as i16;
                let hi = mem.read_u8(linear + 2).unwrap_or(0) as i16;
                let rel = lo | (hi << 8);
                let next_linear = (linear as i32 + 3 + rel as i32) as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_linear });
                (next_linear, pipeline_3(Some(instr_bits), "Fetch JMP", "Decode", &format!("PC += {}", rel)), 3, false)
            }
            0xEB => {
                // JMP rel8 (short)
                let rel = mem.read_u8(linear + 1).unwrap_or(0) as i8;
                let next_linear = (linear as i32 + 2 + rel as i32) as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_linear });
                (next_linear, pipeline_3(Some(instr_bits), "Fetch JMP short", "Decode", "PC += rel8"), 2, false)
            }
            0x74 => {
                // JZ rel8
                let rel = mem.read_u8(linear + 1).unwrap_or(0) as i8;
                let take = (regs[R_FLAGS] & FLAG_Z) != 0;
                let next_linear = if take { (linear as i32 + 2 + rel as i32) as u32 } else { linear + 2 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_linear });
                (next_linear, pipeline_3(Some(instr_bits), "Fetch JZ", "Decode", if take { "Taken" } else { "Not taken" }), 2, false)
            }
            0x75 => {
                // JNZ rel8
                let rel = mem.read_u8(linear + 1).unwrap_or(0) as i8;
                let take = (regs[R_FLAGS] & FLAG_Z) == 0;
                let next_linear = if take { (linear as i32 + 2 + rel as i32) as u32 } else { linear + 2 };
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: next_linear });
                (next_linear, pipeline_3(Some(instr_bits), "Fetch JNZ", "Decode", if take { "Taken" } else { "Not taken" }), 2, false)
            }
            0xE8 => {
                // CALL rel16
                let lo = mem.read_u8(linear + 1).unwrap_or(0) as i16;
                let hi = mem.read_u8(linear + 2).unwrap_or(0) as i16;
                let rel = lo | (hi << 8);
                let ret_addr = linear + 3;
                let sp = get_sp(&regs);
                let sp_new = sp.wrapping_sub(2);
                let addr = linear_addr(ss, sp_new);
                let old_lo = mem.read_u8(addr).unwrap_or(0);
                let old_hi = mem.read_u8(addr + 1).unwrap_or(0);
                mem.write_u8(addr, (ret_addr & 0xFF) as u8).ok();
                mem.write_u8(addr + 1, ((ret_addr >> 8) & 0xFF) as u8).ok();
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: sp_new as u32 });
                undo_log.push(UndoEntry::MemWrite { addr, old_value: old_lo, new_value: (ret_addr & 0xFF) as u8 });
                undo_log.push(UndoEntry::MemWrite { addr: addr + 1, old_value: old_hi, new_value: ((ret_addr >> 8) & 0xFF) as u8 });
                let target_linear = (linear as i32 + 3 + rel as i32) as u32;
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target_linear });
                set_sp(&mut regs, sp_new);
                events.push(TraceEvent::Mem);
                (target_linear, pipeline_3(Some(instr_bits), "Fetch CALL", "Decode", "Push return, jump"), 3, false)
            }
            0xC3 => {
                // RET (near) - pop offset (we store linear in stack for simplicity)
                let sp = get_sp(&regs);
                let addr = linear_addr(ss, sp);
                let lo = mem.read_u8(addr).unwrap_or(0) as u32;
                let hi = mem.read_u8(addr + 1).unwrap_or(0) as u32;
                let target_linear = lo | (hi << 8);
                set_sp(&mut regs, sp.wrapping_add(2));
                undo_log.push(UndoEntry::RegWrite { reg: R_SP, old_value: regs[R_SP], new_value: (sp + 2) as u32 });
                undo_log.push(UndoEntry::Pc { old_value: pc, new_value: target_linear });
                events.push(TraceEvent::Mem);
                (target_linear, pipeline_3(Some(instr_bits), "Fetch RET", "Decode", "Pop PC"), 1, false)
            }
            _ => {
                return StepResult {
                    new_state: state.clone(),
                    events: vec![],
                    undo_log: vec![],
                    cycles_added: 0,
                    halted: false,
                    error: Some(format!("Unknown 8086 opcode 0x{:02X} at linear 0x{:05X}", op, linear)),
                    instruction_bits: Some(instr_bits),
                    pipeline_stages: vec![],
                    io_output: None,
                    io_input_requested: None,
                };
            }
        };

        let next_pc_final = if halted { pc } else { next_linear };

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
                UiBlock { id: "pc".into(), label: "CS:IP".into(), x: 10.0, y: 10.0, width: 70.0, height: 45.0 },
                UiBlock { id: "im".into(), label: "Instr Mem".into(), x: 95.0, y: 10.0, width: 75.0, height: 45.0 },
                UiBlock { id: "ir".into(), label: "IR".into(), x: 185.0, y: 10.0, width: 60.0, height: 45.0 },
                UiBlock { id: "regfile".into(), label: "AX BX CX DX ...".into(), x: 185.0, y: 70.0, width: 95.0, height: 45.0 },
                UiBlock { id: "alu".into(), label: "ALU".into(), x: 295.0, y: 45.0, width: 70.0, height: 45.0 },
                UiBlock { id: "dm".into(), label: "Data Mem".into(), x: 295.0, y: 105.0, width: 75.0, height: 45.0 },
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
            pc_name: "IP (linear)".to_string(),
            reg_names: vec![
                "AX".into(), "BX".into(), "CX".into(), "DX".into(),
                "SI".into(), "DI".into(), "BP".into(), "SP".into(),
                "FLAGS".into(), "CS".into(), "DS".into(), "SS".into(), "ES".into(),
            ],
        }
    }
}

fn parse_8086_instruction(
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
    let args_str: String = if parts.len() > 1 { parts[1..].join(" ") } else { String::new() };
    let args: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();

    source_map.push(SourceMapEntry { pc, line: line_num, column: col });

    match mnemonic.as_str() {
        "NOP" => bytes.push(0x90),
        "HLT" => bytes.push(0xF4),
        "MOV" => {
            if args.len() >= 2 {
                let dst = parse_reg_8086(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid MOV dest".into() })?;
                if let Ok(imm) = parse_imm16(args[1]) {
                    bytes.push(0xB8 + dst as u8);
                    bytes.push((imm & 0xFF) as u8);
                    bytes.push((imm >> 8) as u8);
                } else {
                    let src = parse_reg_8086(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid MOV src".into() })?;
                    bytes.push(0x8B);
                    bytes.push(0xC0 | ((dst as u8) << 3) | (src as u8));
                }
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "MOV needs 2 operands".into() });
            }
        }
        "ADD" => {
            if args.len() >= 2 {
                let dst = parse_reg_8086(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid ADD dest".into() })?;
                let src = parse_reg_8086(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid ADD src".into() })?;
                bytes.push(0x03);
                bytes.push(0xC0 | ((dst as u8) << 3) | (src as u8));
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "ADD needs 2 regs".into() });
            }
        }
        "SUB" => {
            if args.len() >= 2 {
                let dst = parse_reg_8086(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid SUB dest".into() })?;
                let src = parse_reg_8086(args[1]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid SUB src".into() })?;
                bytes.push(0x2B);
                bytes.push(0xC0 | ((dst as u8) << 3) | (src as u8));
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "SUB needs 2 regs".into() });
            }
        }
        "PUSH" => {
            if args.len() >= 1 {
                let r = parse_reg_8086(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid PUSH reg".into() })?;
                bytes.push(0x50 + r as u8);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "PUSH needs reg".into() });
            }
        }
        "POP" => {
            if args.len() >= 1 {
                let r = parse_reg_8086(args[0]).ok_or_else(|| AssemblerError { line: line_num, column: col, message: "Invalid POP reg".into() })?;
                bytes.push(0x58 + r as u8);
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "POP needs reg".into() });
            }
        }
        "JMP" => {
            if args.len() >= 1 {
                if let Ok(addr) = parse_imm16(args[0]) {
                    let rel = (addr as i32) - (bytes.len() as i32) - 3;
                    if rel >= -128 && rel <= 127 {
                        bytes.push(0xEB);
                        bytes.push(rel as u8);
                    } else {
                        bytes.push(0xE9);
                        bytes.push((rel & 0xFF) as u8);
                        bytes.push(((rel >> 8) & 0xFF) as u8);
                    }
                } else {
                    bytes.push(0xE9);
                    let off = bytes.len();
                    bytes.push(0);
                    bytes.push(0);
                    pending_refs.push((off, args[0].to_string(), true));
                }
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "JMP needs target".into() });
            }
        }
        "JZ" | "JNZ" => {
            if args.len() >= 1 {
                let op = if mnemonic == "JZ" { 0x74 } else { 0x75 };
                bytes.push(op);
                let off = bytes.len();
                bytes.push(0);
                pending_refs.push((off, args[0].to_string(), false));
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "JZ/JNZ needs label".into() });
            }
        }
        "CALL" => {
            if args.len() >= 1 {
                bytes.push(0xE8);
                let off = bytes.len();
                bytes.push(0);
                bytes.push(0);
                pending_refs.push((off, args[0].to_string(), true));
            } else {
                return Err(AssemblerError { line: line_num, column: col, message: "CALL needs label".into() });
            }
        }
        "RET" => bytes.push(0xC3),
        _ => return Err(AssemblerError { line: line_num, column: col, message: format!("Unknown mnemonic: {}", mnemonic) }),
    }
    Ok(())
}
