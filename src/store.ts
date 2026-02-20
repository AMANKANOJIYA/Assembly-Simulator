import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog, open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  SimulatorStateSnapshot,
  AssemblerError,
  UiSchema,
  RegisterSchema,
} from "./types";

export interface PanelVisibility {
  code: boolean;
  arch: boolean;
  trace: boolean;
  registers: boolean;
  memory: boolean;
  input: boolean;
  output: boolean;
}

export type PanelId = "registers" | "memory" | "trace";

export interface AsimFileData {
  version: number;
  arch: string;
  source: string;
  memory_size: number;
  breakpoints: number[];
  entry_point?: string;
  speed: number;
  max_cycle_limit?: number;
}

export interface AppState {
  source: string;
  arch: string;
  speed: number; // ms per tick
  clockMHz: number; // clock frequency in MHz for real execution time
  memorySize: number;
  filePath: string | null;
  breakpoints: number[];
  maxCycleLimit: number | null; // null = unlimited
  snapshot: SimulatorStateSnapshot | null;
  uiSchema: UiSchema | null;
  registerSchema: RegisterSchema | null;
  errors: AssemblerError[];
  toast: { message: string; type: "error" | "info" } | null;
  runIntervalId: number | null;
  panelVisibility: PanelVisibility;
  archExpanded: boolean;
  blockPositions: Record<string, { x: number; y: number }>;
  panelOrder: PanelId[];
  customizeMode: boolean;
  cycleHistory: { cycle: number; stage: string; instructionBits?: number; action: string }[];
  cycleGraphOpen: boolean;
  helpOpen: boolean;
  settingsOpen: boolean;
}

export interface AppActions {
  setSource: (s: string) => void;
  setArch: (a: string) => void;
  setSpeed: (s: number) => void;
  setClockMHz: (m: number) => void;
  setMemorySize: (s: number) => void;
  setSnapshot: (s: SimulatorStateSnapshot | null) => void;
  setToast: (t: AppState["toast"]) => void;
  assemble: () => Promise<boolean>;
  run: () => void;
  runAfterInput: () => void;
  pause: () => void;
  stepForward: () => Promise<void>;
  stepForwardWithInput: (input: string) => Promise<void>;
  stepBack: () => Promise<void>;
  reset: () => Promise<void>;
  refreshState: () => Promise<void>;
  loadSchemas: (archOverride?: string) => Promise<void>;
  setRunIntervalId: (id: number | null) => void;
  setPanelVisibility: (key: keyof PanelVisibility, value: boolean) => void;
  setArchExpanded: (v: boolean) => void;
  setBlockPosition: (id: string, x: number, y: number) => void;
  resetBlockPositions: () => void;
  setPanelOrder: (order: PanelId[]) => void;
  movePanel: (id: PanelId, direction: "up" | "down") => void;
  setCustomizeMode: (v: boolean) => void;
  setCycleHistoryGraphOpen: (v: boolean) => void;
  setHelpOpen: (v: boolean) => void;
  setSettingsOpen: (v: boolean) => void;
  setFilePath: (p: string | null) => void;
  setBreakpoints: (b: number[]) => void;
  toggleBreakpoint: (addr: number) => void;
  setMaxCycleLimit: (n: number | null) => void;
  saveFile: () => Promise<boolean>;
  loadFile: () => Promise<boolean>;
  newFile: () => void;
}

export const useStore = create<AppState & AppActions>((set, get) => ({
  source: "",
  arch: "RV32I",
  speed: 100,
  clockMHz: 1000,
  memorySize: 65536,
  filePath: null,
  breakpoints: [],
  maxCycleLimit: null,
  snapshot: null,
  uiSchema: null,
  registerSchema: null,
  errors: [],
  toast: null,
  runIntervalId: null,
  panelVisibility: {
    code: true,
    arch: true,
    trace: true,
    registers: true,
    memory: true,
    input: true,
    output: true,
  },
  archExpanded: false,
  blockPositions: {},
  panelOrder: ["registers", "memory", "trace"],
  customizeMode: false,
  cycleHistory: [],
  cycleGraphOpen: false,
  helpOpen: false,
  settingsOpen: false,

  setSource: (s) => set({ source: s }),
  setArch: (a) => set({ arch: a }),
  setSpeed: (s) => set({ speed: s }),
  setClockMHz: (m) => set({ clockMHz: m }),
  setMemorySize: (s) => set({ memorySize: s }),
  setSnapshot: (s) => set({ snapshot: s }),
  setToast: (t) => set({ toast: t }),

  assemble: async () => {
    const { source, arch } = get();
    try {
      const result = await invoke<{ errors: AssemblerError[]; ok: boolean }>(
        "assemble_check",
        { source, arch }
      );
      if (!result.ok) {
        set({ errors: result.errors });
        get().setToast({
          message: `Assembler errors: ${result.errors.length} error(s)`,
          type: "error",
        });
        return false;
      }
      try {
        const { memorySize } = get();
        await invoke("load_program", { source, arch, memorySize });
      } catch (e) {
        get().setToast({ message: String(e), type: "error" });
        return false;
      }
      set({ errors: [], cycleHistory: [] });
      await get().refreshState();
      get().setToast({ message: "Assembled successfully", type: "info" });
      return true;
    } catch (e) {
      const err = String(e);
      try {
        const parsed = JSON.parse(err) as AssemblerError[];
        set({ errors: parsed });
      } catch {
        set({ errors: [] });
      }
      get().setToast({ message: err, type: "error" });
      return false;
    }
  },

  run: async () => {
    const { snapshot, source, arch } = get();
    if (snapshot?.halted) return;

    // Ensure program is loaded before run (in case user skipped Assemble)
    try {
      const check = await invoke<{ ok: boolean }>("assemble_check", { source, arch });
      if (!check.ok) {
        get().setToast({ message: "Assemble first: fix errors in the code", type: "error" });
        return;
      }
      const { memorySize, breakpoints } = get();
      await invoke("load_program", { source, arch, memorySize });
      await invoke("set_breakpoints", { addrs: breakpoints });
      set({ errors: [], cycleHistory: [] });
      await get().refreshState();
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
      return;
    }

    await invoke("set_running", { running: true });
    get().runAfterInput();
  },

  runAfterInput: () => {
    invoke("set_running", { running: true });
    const runLoop = async () => {
      try {
        const tickResult = await invoke<{ io_input_requested?: unknown } | null>("run_tick");
        await get().refreshState();
        const s = get().snapshot;
        const { breakpoints, maxCycleLimit } = get();
        if (tickResult?.io_input_requested) {
          get().pause();
          return;
        }
        if (s?.halted || s?.run_state === "HALTED" || s?.run_state === "ERROR") {
          get().pause();
          if (s?.run_state === "ERROR" && s?.run_error) {
            get().setToast({ message: s.run_error, type: "error" });
          }
          return;
        }
        if (s && breakpoints.includes(s.state.pc)) {
          get().pause();
          get().setToast({ message: "Breakpoint hit", type: "info" });
          return;
        }
        if (s && maxCycleLimit != null && s.total_cycles >= maxCycleLimit) {
          get().pause();
          get().setToast({ message: `Max cycle limit (${maxCycleLimit}) reached`, type: "info" });
          return;
        }
        const id = window.setTimeout(runLoop, get().speed);
        set({ runIntervalId: id });
      } catch (e) {
        get().pause();
        get().setToast({ message: String(e), type: "error" });
      }
    };
    const id = window.setTimeout(runLoop, get().speed);
    set({ runIntervalId: id });
  },

  pause: () => {
    const { runIntervalId } = get();
    if (runIntervalId != null) {
      clearTimeout(runIntervalId);
      set({ runIntervalId: null });
    }
    invoke("set_running", { running: false });
    get().refreshState();
  },

  stepForward: async () => {
    try {
      await invoke("step_forward");
      await get().refreshState();
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
    }
  },

  stepForwardWithInput: async (input: string) => {
    try {
      await invoke("step_forward_with_input", { input });
      await get().refreshState();
      const s = get().snapshot;
      if (s && !s.halted && !s.io_input_requested && get().runIntervalId == null) {
        get().runAfterInput();
      }
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
    }
  },

  stepBack: async () => {
    try {
      await invoke("step_back");
      await get().refreshState();
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
    }
  },

  reset: async () => {
    const { source, arch, memorySize } = get();
    try {
      const check = await invoke<{ ok: boolean }>("assemble_check", { source, arch });
      if (!check.ok) {
        get().setToast({ message: "Fix assembly errors before reset", type: "error" });
        return;
      }
      await invoke("reset_with_program", { source, arch, memorySize });
      set({ errors: [], cycleHistory: [] });
      await get().refreshState();
      get().setToast({ message: "Reset complete", type: "info" });
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
    }
  },

  refreshState: async () => {
    try {
      const s = await invoke<SimulatorStateSnapshot>("get_state");
      set(() => {
        const cycleHistory = (s.cycle_details ?? []).map((d) => ({
          cycle: d.cycle,
          stage: d.stage,
          instructionBits: d.instruction_bits,
          action: d.action ?? "",
        }));
        return { snapshot: s, cycleHistory };
      });
    } catch {
      set({ snapshot: null });
    }
  },

  loadSchemas: async (archOverride?: string) => {
    const { arch, memorySize } = get();
    const targetArch = archOverride ?? arch;
    try {
      const [ui, reg] = await Promise.all([
        invoke<UiSchema>("get_ui_schema", { arch: targetArch }),
        invoke<RegisterSchema>("get_register_schema", { arch: targetArch }),
      ]);
      set({ uiSchema: ui, registerSchema: reg, arch: targetArch });
      await invoke("reset_for_arch_change", { arch: targetArch, memorySize });
      await get().refreshState();
    } catch {
      set({ uiSchema: null, registerSchema: null });
    }
  },

  setRunIntervalId: (id) => set({ runIntervalId: id }),

  setPanelVisibility: (key, value) =>
    set((s) => ({
      panelVisibility: { ...s.panelVisibility, [key]: value },
    })),

  setArchExpanded: (v) => set({ archExpanded: v }),

  setBlockPosition: (id, x, y) =>
    set((s) => ({
      blockPositions: { ...s.blockPositions, [id]: { x, y } },
    })),

  resetBlockPositions: () => set({ blockPositions: {} }),

  setPanelOrder: (order) => set({ panelOrder: order }),

  movePanel: (id, direction) =>
    set((s) => {
      const arr = [...s.panelOrder];
      const i = arr.indexOf(id);
      if (i < 0) return s;
      const j = direction === "up" ? i - 1 : i + 1;
      if (j < 0 || j >= arr.length) return s;
      [arr[i], arr[j]] = [arr[j], arr[i]];
      return { panelOrder: arr };
    }),

  setCustomizeMode: (v) => set({ customizeMode: v }),

  setCycleHistoryGraphOpen: (v) => set({ cycleGraphOpen: v }),
  setHelpOpen: (v) => set({ helpOpen: v }),
  setSettingsOpen: (v) => set({ settingsOpen: v }),
  setFilePath: (p) => set({ filePath: p }),
  setBreakpoints: (b) => set({ breakpoints: b }),
  toggleBreakpoint: (addr) =>
    set((s) => {
      const idx = s.breakpoints.indexOf(addr);
      const next =
        idx >= 0
          ? [...s.breakpoints.slice(0, idx), ...s.breakpoints.slice(idx + 1)]
          : [...s.breakpoints, addr].sort((a, b) => a - b);
      return { breakpoints: next };
    }),
  setMaxCycleLimit: (n) => set({ maxCycleLimit: n }),
  saveFile: async () => {
    const { source, arch, memorySize, breakpoints, speed, maxCycleLimit } = get();
    try {
      const path = await saveDialog({
        filters: [{ name: "Assembly Simulator", extensions: ["asim"] }],
        title: "Save Project",
      });
      if (path == null) return false;
      await invoke("write_asim_file", {
        path,
        data: {
          version: 1,
          arch,
          source,
          memory_size: memorySize,
          breakpoints,
          entry_point: "_start",
          speed,
          max_cycle_limit: maxCycleLimit ?? undefined,
        },
      });
      set({ filePath: path });
      get().setToast({ message: "Saved successfully", type: "info" });
      return true;
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
      return false;
    }
  },
  loadFile: async () => {
    try {
      const path = await openDialog({
        filters: [{ name: "Assembly Simulator", extensions: ["asim"] }],
        title: "Open Project",
        multiple: false,
      });
      if (path == null) return false;
      const data = await invoke<AsimFileData>("read_asim_file", { path });
      set({
        filePath: path,
        source: data.source,
        arch: data.arch,
        memorySize: data.memory_size ?? 65536,
        breakpoints: data.breakpoints ?? [],
        speed: data.speed ?? 100,
        maxCycleLimit: data.max_cycle_limit ?? null,
        errors: [],
        cycleHistory: [],
      });
      await get().loadSchemas();
      await get().refreshState();
      get().setToast({ message: "Loaded successfully", type: "info" });
      return true;
    } catch (e) {
      get().setToast({ message: String(e), type: "error" });
      return false;
    }
  },
  newFile: () => {
    set({
      source: "",
      arch: "RV32I",
      filePath: null,
      breakpoints: [],
      errors: [],
      cycleHistory: [],
    });
    get().setToast({ message: "New file", type: "info" });
  },
}));

const PERSIST_KEY = "asim-session";
export function loadSession() {
  try {
    const raw = localStorage.getItem(PERSIST_KEY);
    if (!raw) return;
    const data = JSON.parse(raw) as Partial<{
      panelVisibility: PanelVisibility;
      panelOrder: PanelId[];
      speed: number;
      memorySize: number;
    }>;
    useStore.setState((s) => ({
      panelVisibility: data.panelVisibility
        ? { ...s.panelVisibility, ...data.panelVisibility }
        : s.panelVisibility,
      panelOrder: data.panelOrder ?? s.panelOrder,
      speed: data.speed ?? s.speed,
      memorySize: data.memorySize ?? s.memorySize,
    }));
  } catch {
    // ignore
  }
}
export function saveSession() {
  try {
    const s = useStore.getState();
    localStorage.setItem(
      PERSIST_KEY,
      JSON.stringify({
        panelVisibility: s.panelVisibility,
        panelOrder: s.panelOrder,
        speed: s.speed,
        memorySize: s.memorySize,
      })
    );
  } catch {
    // ignore
  }
}
