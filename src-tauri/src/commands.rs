//! Tauri commands - bridge between frontend and simulator.

use crate::asim::AsimFile;
use crate::plugin::{InputRequest, *};
use std::fs;
use crate::simulator::{CycleDetail, RunState, Simulator};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref SIM: Mutex<Simulator> = Mutex::new(Simulator::new());
}

#[tauri::command]
pub fn assemble(source: String, arch: String) -> Result<AssembleResponse, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    let image = sim.assemble(&source, &arch)?;
    Ok(AssembleResponse {
        bytes: image.bytes,
        entry_pc: image.entry_pc,
        source_map: image.source_map,
        errors: vec![],
    })
}

#[tauri::command]
pub fn assemble_check(source: String, arch: String) -> AssembleCheckResponse {
    let mut sim = SIM.lock().unwrap();
    sim.arch = arch;
    let image = sim.get_plugin().assemble(&source);
    let ok = image.errors.is_empty();
    AssembleCheckResponse {
        errors: image.errors,
        ok,
    }
}

#[tauri::command]
pub fn reset(arch: String, config: ResetConfig) -> Result<CpuState, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.arch = arch;
    Ok(sim.reset(&config))
}

#[tauri::command]
pub fn reset_with_program(source: String, arch: String, memory_size: usize) -> Result<CpuState, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    let image = sim.assemble(&source, &arch)?;
    sim.arch = arch;
    let config = ResetConfig { memory_size };
    Ok(sim.reset(&config))
}

#[tauri::command]
pub fn reset_for_arch_change(arch: String, memory_size: usize) -> Result<CpuState, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.program_image = None;
    sim.arch = arch;
    let config = ResetConfig { memory_size };
    Ok(sim.reset(&config))
}

#[tauri::command]
pub fn step_forward() -> Result<StepResult, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.step_forward()
}

#[tauri::command]
pub fn step_forward_with_input(input: String) -> Result<StepResult, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.step_forward_with_input(Some(&input))
}

#[tauri::command]
pub fn step_back() -> Result<(), String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.step_back()
}

#[tauri::command]
pub fn run_tick() -> Result<Option<StepResult>, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.run_tick()
}

#[tauri::command]
pub fn set_running(running: bool) {
    let mut sim = SIM.lock().unwrap();
    sim.set_run_state(if running {
        RunState::Running
    } else {
        RunState::Paused
    });
}

#[tauri::command]
pub fn get_state() -> Result<SimulatorStateSnapshot, String> {
    let sim = SIM.lock().map_err(|e| e.to_string())?;
    let source_map = sim
        .program_image
        .as_ref()
        .map(|img| img.source_map.clone())
        .unwrap_or_default();
    Ok(SimulatorStateSnapshot {
        state: sim.state.clone(),
        memory: sim.memory.data().to_vec(),
        memory_size: sim.memory.size(),
        total_cycles: sim.total_cycles,
        run_state: format_run_state(&sim.run_state),
        trace_events: sim.trace_events.clone(),
        can_step_back: !sim.undo_stack.is_empty(),
        halted: sim.state.halted,
        last_instruction: sim.last_instruction,
        run_error: get_run_error(&sim.run_state),
        cycle_details: sim.cycle_details.clone(),
        source_map,
        io_output: sim.io_output.clone(),
        io_input_requested: sim.pending_input_request.clone(),
    })
}

fn format_run_state(rs: &RunState) -> String {
    match rs {
        RunState::Idle => "IDLE",
        RunState::Running => "RUNNING",
        RunState::Paused => "PAUSED",
        RunState::Halted => "HALTED",
        RunState::Error(_) => "ERROR",
    }
    .to_string()
}

fn get_run_error(rs: &RunState) -> Option<String> {
    match rs {
        RunState::Error(s) => Some(s.clone()),
        _ => None,
    }
}

#[tauri::command]
pub fn set_memory_size(size: usize) -> Result<(), String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    sim.set_memory_size(size);
    Ok(())
}

#[tauri::command]
pub fn set_breakpoints(addrs: Vec<u32>) {
    let mut sim = SIM.lock().unwrap();
    sim.set_breakpoints(addrs);
}

#[tauri::command]
pub fn load_program(source: String, arch: String, memory_size: Option<usize>) -> Result<LoadProgramResponse, String> {
    let mut sim = SIM.lock().map_err(|e| e.to_string())?;
    let image = sim.assemble(&source, &arch)?;
    let config = ResetConfig {
        memory_size: memory_size.unwrap_or(64 * 1024),
    };
    sim.reset(&config);
    Ok(LoadProgramResponse {
        entry_pc: image.entry_pc,
        bytes_len: image.bytes.len(),
    })
}

#[tauri::command]
pub fn get_ui_schema(arch: String) -> UiSchema {
    let mut sim = SIM.lock().unwrap();
    sim.arch = arch;
    sim.get_plugin().ui_schema()
}

#[tauri::command]
pub fn get_register_schema(arch: String) -> RegisterSchema {
    let mut sim = SIM.lock().unwrap();
    sim.arch = arch;
    sim.get_plugin().register_schema()
}

/// Write ASIM file to path (path comes from frontend dialog - non-blocking)
#[tauri::command]
pub fn write_asim_file(path: String, data: AsimFile) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Read ASIM file from path (path comes from frontend dialog - non-blocking)
#[tauri::command]
pub fn read_asim_file(path: String) -> Result<AsimFile, String> {
    let json = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: AsimFile = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    Ok(data)
}

#[derive(serde::Serialize)]
pub struct AssembleResponse {
    pub bytes: Vec<u8>,
    pub entry_pc: u32,
    pub source_map: Vec<SourceMapEntry>,
    pub errors: Vec<AssemblerError>,
}

#[derive(serde::Serialize)]
pub struct AssembleCheckResponse {
    pub errors: Vec<AssemblerError>,
    pub ok: bool,
}

#[derive(serde::Serialize)]
pub struct SimulatorStateSnapshot {
    pub state: CpuState,
    pub memory: Vec<u8>,
    pub memory_size: usize,
    pub total_cycles: u64,
    pub run_state: String,
    pub trace_events: Vec<TraceEvent>,
    pub can_step_back: bool,
    pub halted: bool,
    pub last_instruction: Option<u32>,
    pub run_error: Option<String>,
    pub cycle_details: Vec<CycleDetail>,
    pub source_map: Vec<SourceMapEntry>,
    pub io_output: String,
    pub io_input_requested: Option<InputRequest>,
}

#[derive(serde::Serialize)]
pub struct LoadProgramResponse {
    pub entry_pc: u32,
    pub bytes_len: usize,
}
