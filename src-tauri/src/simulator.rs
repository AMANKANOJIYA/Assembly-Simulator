//! Simulator state machine: holds CPU state, memory, undo stack for step-back.

use crate::memory::Memory;
use crate::plugin::{InputRequest, I6502Plugin, I8085Plugin, I8086Plugin, Lc3Plugin, MipsPlugin, Rv32iPlugin, *};

#[derive(Clone, Debug, serde::Serialize)]
pub struct CycleDetail {
    pub cycle: u64,
    pub stage: String,
    pub instruction_bits: Option<u32>,
    pub action: String,
}

pub struct Simulator {
    pub arch: String,
    pub state: CpuState,
    pub memory: Memory,
    pub program_image: Option<ProgramImage>,
    pub undo_stack: Vec<UndoSnapshot>,
    pub trace_events: Vec<TraceEvent>,
    pub total_cycles: u64,
    pub run_state: RunState,
    pub last_instruction: Option<u32>,
    pub cycle_details: Vec<CycleDetail>,
    pub io_output: String,
    /// Pending input request – set when step returns io_input_requested without applying
    pub pending_input_request: Option<InputRequest>,
    /// Breakpoints (PC values) – run_tick pauses when PC hits one
    pub breakpoints: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
    Idle,
    Running,
    Paused,
    Halted,
    Error(String),
}

#[derive(Clone)]
pub struct UndoSnapshot {
    pub state: CpuState,
    pub undo_log: Vec<UndoEntry>,
    pub cycles_added: u64,
    /// Number of chars appended to io_output by this step (for step-back undo)
    pub io_output_len: usize,
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            arch: "RV32I".to_string(),
            state: CpuState {
                pc: 0,
                regs: vec![0u32; 32],
                halted: false,
            },
            memory: Memory::new(64 * 1024),
            program_image: None,
            undo_stack: Vec::new(),
            trace_events: Vec::new(),
            total_cycles: 0,
            run_state: RunState::Idle,
            last_instruction: None,
            cycle_details: Vec::new(),
            io_output: String::new(),
            pending_input_request: None,
            breakpoints: Vec::new(),
        }
    }

    pub fn get_plugin(&self) -> &'static dyn ArchitecturePlugin {
        match self.arch.to_uppercase().as_str() {
            "LC3" => {
                static LC3: std::sync::OnceLock<Lc3Plugin> = std::sync::OnceLock::new();
                LC3.get_or_init(Lc3Plugin::new)
            }
            "MIPS" => {
                static MIPS: std::sync::OnceLock<MipsPlugin> = std::sync::OnceLock::new();
                MIPS.get_or_init(MipsPlugin::new)
            }
            "8085" => {
                static I8085: std::sync::OnceLock<I8085Plugin> = std::sync::OnceLock::new();
                I8085.get_or_init(I8085Plugin::new)
            }
            "6502" => {
                static I6502: std::sync::OnceLock<I6502Plugin> = std::sync::OnceLock::new();
                I6502.get_or_init(I6502Plugin::new)
            }
            "8086" => {
                static I8086: std::sync::OnceLock<I8086Plugin> = std::sync::OnceLock::new();
                I8086.get_or_init(I8086Plugin::new)
            }
            _ => {
                static RV32I: std::sync::OnceLock<Rv32iPlugin> = std::sync::OnceLock::new();
                RV32I.get_or_init(Rv32iPlugin::new)
            }
        }
    }

    pub fn assemble(&mut self, source: &str, arch: &str) -> Result<ProgramImage, String> {
        self.arch = arch.to_string();
        let plugin = self.get_plugin();
        let image = plugin.assemble(source);
        if !image.errors.is_empty() {
            return Err(serde_json::to_string(&image.errors).unwrap_or_default());
        }
        self.program_image = Some(image.clone());
        Ok(image)
    }

    pub fn reset(&mut self, config: &ResetConfig) -> CpuState {
        let plugin = self.get_plugin();
        self.state = plugin.reset(config);
        self.memory = Memory::new(config.memory_size);
        if let Some(ref img) = self.program_image {
            let _ = self.memory.load_program(img.entry_pc, &img.bytes);
            self.state.pc = img.entry_pc;
        }
        self.undo_stack.clear();
        self.trace_events.clear();
        self.total_cycles = 0;
        self.last_instruction = None;
        self.cycle_details.clear();
        self.run_state = RunState::Idle;
        self.io_output.clear();
        self.pending_input_request = None;
        self.state.clone()
    }

    fn apply_undo_forward(&mut self, log: &[UndoEntry]) {
        for entry in log {
            match entry {
                UndoEntry::RegWrite { reg, new_value, .. } => {
                    if *reg < self.state.regs.len() {
                        self.state.regs[*reg] = *new_value;
                    }
                }
                UndoEntry::MemWrite { addr, new_value, .. } => {
                    if (*addr as usize) < self.memory.size() {
                        let _ = self.memory.write_u8(*addr, *new_value);
                    }
                }
                UndoEntry::Pc { new_value, .. } => {
                    self.state.pc = *new_value;
                }
            }
        }
    }

    fn apply_undo_reverse(&mut self, log: &[UndoEntry]) {
        for entry in log.iter().rev() {
            match entry {
                UndoEntry::RegWrite { reg, old_value, .. } => {
                    if *reg < self.state.regs.len() {
                        self.state.regs[*reg] = *old_value;
                    }
                }
                UndoEntry::MemWrite { addr, old_value, .. } => {
                    if (*addr as usize) < self.memory.size() {
                        let _ = self.memory.write_u8(*addr, *old_value);
                    }
                }
                UndoEntry::Pc { old_value, .. } => {
                    self.state.pc = *old_value;
                }
            }
        }
    }

    pub fn step_forward(&mut self) -> Result<StepResult, String> {
        self.step_forward_with_input(None)
    }

    pub fn step_forward_with_input(&mut self, input: Option<&str>) -> Result<StepResult, String> {
        if self.state.halted {
            return Err("CPU halted".to_string());
        }
        let plugin = self.get_plugin();
        let result = plugin.step(
            &self.state,
            self.memory.data(),
            StepMode::Instruction,
            input,
        );
        if let Some(ref e) = result.error {
            self.run_state = RunState::Error(e.clone());
            return Err(e.clone());
        }
        // Input requested and no input provided – don't apply, store pending, return
        if result.io_input_requested.is_some() && input.is_none() {
            self.pending_input_request = result.io_input_requested.clone();
            self.run_state = RunState::Paused;
            return Ok(result);
        }
        self.pending_input_request = None;
        let io_output_len = result.io_output.as_ref().map(|s| s.len()).unwrap_or(0);
        // Push undo snapshot before applying
        self.undo_stack.push(UndoSnapshot {
            state: self.state.clone(),
            undo_log: result.undo_log.clone(),
            cycles_added: result.cycles_added,
            io_output_len,
        });
        self.apply_undo_forward(&result.undo_log);
        self.state = result.new_state.clone();
        self.total_cycles += result.cycles_added;
        self.trace_events = result.events.clone();
        self.last_instruction = result.instruction_bits;
        for (i, pi) in result.pipeline_stages.iter().enumerate() {
            let c = self.total_cycles - result.cycles_added + 1 + i as u64;
            self.cycle_details.push(CycleDetail {
                cycle: c,
                stage: pi.stage.clone(),
                instruction_bits: pi.instruction_bits,
                action: pi.action.clone(),
            });
        }
        if result.halted {
            self.run_state = RunState::Halted;
        }
        if let Some(ref s) = result.io_output {
            self.io_output.push_str(s);
        }
        Ok(result)
    }

    pub fn step_back(&mut self) -> Result<(), String> {
        let snapshot = self
            .undo_stack
            .pop()
            .ok_or("No step to undo".to_string())?;
        self.state = snapshot.state.clone();
        self.apply_undo_reverse(&snapshot.undo_log);
        self.total_cycles = self.total_cycles.saturating_sub(snapshot.cycles_added);
        let trunc = self.cycle_details.len().saturating_sub(snapshot.cycles_added as usize);
        self.cycle_details.truncate(trunc);
        if snapshot.io_output_len > 0 && self.io_output.len() >= snapshot.io_output_len {
            self.io_output.truncate(self.io_output.len() - snapshot.io_output_len);
        }
        self.run_state = RunState::Paused;
        self.pending_input_request = None;
        if self.state.halted {
            self.run_state = RunState::Halted;
        }
        Ok(())
    }

    pub fn run_tick(&mut self) -> Result<Option<StepResult>, String> {
        match self.run_state {
            RunState::Running => {
                if self.state.halted {
                    self.run_state = RunState::Halted;
                    return Ok(None);
                }
                if self.breakpoints.contains(&self.state.pc) {
                    self.run_state = RunState::Paused;
                    return Ok(None);
                }
                match self.step_forward() {
                    Ok(r) => {
                        if r.io_input_requested.is_some() {
                            self.run_state = RunState::Paused;
                        } else if self.breakpoints.contains(&r.new_state.pc) {
                            self.run_state = RunState::Paused;
                        }
                        Ok(Some(r))
                    }
                    Err(e) => {
                        self.run_state = RunState::Error(e.clone());
                        Err(e)
                    }
                }
            }
            _ => Ok(None),
        }
    }

    pub fn set_breakpoints(&mut self, addrs: Vec<u32>) {
        self.breakpoints = addrs;
    }

    pub fn set_run_state(&mut self, s: RunState) {
        self.run_state = s;
    }

    pub fn set_memory_size(&mut self, size: usize) {
        self.memory = Memory::new(size);
        if let Some(ref img) = self.program_image {
            let _ = self.memory.load_program(img.entry_pc, &img.bytes);
        }
    }
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}
