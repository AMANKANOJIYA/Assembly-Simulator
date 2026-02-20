export interface AssemblerError {
  line: number;
  column: number;
  message: string;
}

export interface CpuState {
  pc: number;
  regs: number[];
  halted: boolean;
}

export type TraceEvent =
  | "FETCH"
  | "DECODE"
  | "ALU"
  | "MEM"
  | "REG_WRITE"
  | "HALTED";

export interface CycleDetail {
  cycle: number;
  stage: string;
  instruction_bits?: number;
  action: string;
}

export interface SourceMapEntry {
  pc: number;
  line: number;
  column: number;
}

export interface InputRequest {
  kind: string;
  prompt: string;
  max_length?: number;
}

export interface SimulatorStateSnapshot {
  state: CpuState;
  memory: number[];
  memory_size: number;
  total_cycles: number;
  run_state: string;
  trace_events: TraceEvent[];
  can_step_back: boolean;
  halted: boolean;
  last_instruction?: number;
  run_error?: string | null;
  cycle_details: CycleDetail[];
  source_map?: SourceMapEntry[];
  io_output?: string;
  io_input_requested?: InputRequest | null;
}

export interface UiBlock {
  id: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface UiConnection {
  from: string;
  to: string;
}

export interface UiSchema {
  blocks: UiBlock[];
  connections: UiConnection[];
}

export interface RegisterSchema {
  pc_name: string;
  reg_names: string[];
}
