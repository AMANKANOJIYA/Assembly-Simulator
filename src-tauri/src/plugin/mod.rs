//! Architecture plugin system - defines the trait and schemas for extensible architectures.

pub mod adapter;
pub mod lc3;
pub mod mips;
pub mod rv32i;

pub use lc3::Lc3Plugin;
pub use mips::MipsPlugin;
pub use rv32i::Rv32iPlugin;

use serde::{Deserialize, Serialize};

/// Assemble result - program bytes with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramImage {
    pub bytes: Vec<u8>,
    pub entry_pc: u32,
    pub source_map: Vec<SourceMapEntry>,
    pub errors: Vec<AssemblerError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapEntry {
    pub pc: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssemblerError {
    pub line: u32,
    pub column: u32,
    pub message: String,
}

/// CPU state snapshot - registers and PC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuState {
    pub pc: u32,
    pub regs: Vec<u32>,
    pub halted: bool,
}

/// Reset configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetConfig {
    pub memory_size: usize,
}

impl Default for ResetConfig {
    fn default() -> Self {
        Self {
            memory_size: 64 * 1024, // 64KB default
        }
    }
}

/// Pipeline stage / event type for trace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TraceEvent {
    Fetch,
    Decode,
    Alu,
    Mem,
    RegWrite,
    Halted,
}

/// Single undoable change - used for step-back
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum UndoEntry {
    RegWrite { reg: usize, old_value: u32, new_value: u32 },
    MemWrite { addr: u32, old_value: u8, new_value: u8 },
    Pc { old_value: u32, new_value: u32 },
}

/// One pipeline cycle - stage name, instruction, and stage-specific action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCycleInfo {
    pub stage: String,
    pub instruction_bits: Option<u32>,
    /// What this stage does (e.g. "Load 0x00a00093 from IMem[PC]", "ALU: x0 + 10 = 10")
    pub action: String,
}

/// Request for user input when program hits TRAP IN / read syscall
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRequest {
    /// Input type: "char" (1 char), "int" (integer), "string" (text)
    pub kind: String,
    /// Human-readable prompt
    pub prompt: String,
    /// Max length for strings
    #[serde(default)]
    pub max_length: Option<u32>,
}

/// Result of a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub new_state: CpuState,
    pub events: Vec<TraceEvent>,
    pub undo_log: Vec<UndoEntry>,
    pub cycles_added: u64,
    pub halted: bool,
    pub error: Option<String>,
    /// 32-bit instruction word that was executed
    pub instruction_bits: Option<u32>,
    /// Per-cycle pipeline stages (e.g. 5 stages: Fetch, Decode, Execute, Memory, Write-back)
    pub pipeline_stages: Vec<PipelineCycleInfo>,
    /// Output from ecall print (e.g. print int, print char)
    #[serde(default)]
    pub io_output: Option<String>,
    /// Set when program needs input (TRAP IN, read syscall) – don't advance, show input UI
    #[serde(default)]
    pub io_input_requested: Option<InputRequest>,
}

/// UI block for diagram - describes a visual block in the architecture diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiBlock {
    pub id: String,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Register schema - describes registers for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterSchema {
    pub pc_name: String,
    pub reg_names: Vec<String>,
}

/// UI schema - blocks and layout for diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSchema {
    pub blocks: Vec<UiBlock>,
    pub connections: Vec<UiConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConnection {
    pub from: String,
    pub to: String,
}

/// Architecture plugin trait - each architecture implements this
pub trait ArchitecturePlugin: Send + Sync {
    fn name(&self) -> &str;

    fn assemble(&self, source: &str) -> ProgramImage;

    fn reset(&self, config: &ResetConfig) -> CpuState;

    /// Execute one step. If `input` is Some, use it to complete a pending read (TRAP IN / syscall).
    fn step(&self, state: &CpuState, memory: &[u8], mode: StepMode, input: Option<&str>) -> StepResult;

    fn ui_schema(&self) -> UiSchema;

    fn register_schema(&self) -> RegisterSchema;
}

/// Step mode - instruction-level vs stage-level (for future)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepMode {
    Instruction,
    Stage,
}
