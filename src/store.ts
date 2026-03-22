import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog, open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  SimulatorStateSnapshot,
  AssemblerError,
  UiSchema,
  RegisterSchema,
} from "./types";
import type { UiFontId, MonoFontId, UiDensity } from "./appearance";

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

export type ThemeMode = "dark" | "light";

/** One editor tab (VS Code–style multi-file) */
export interface EditorTab {
  id: string;
  title: string;
  filePath: string | null;
}

const INITIAL_EDITOR_TAB_ID = "tab-main";

export interface AsimFileData {
  version: number;
  arch: string;
  source: string;
  memory_size: number;
  breakpoints: number[];
  entry_point?: string;
  speed: number;
  max_cycle_limit?: number;
  panel_visibility?: Partial<PanelVisibility>;
}

export interface AppState {
  /** Open editor tabs (titles + paths) */
  editorTabs: EditorTab[];
  activeEditorTabId: string;
  /** Per-tab source buffers */
  tabBuffers: Record<string, string>;
  arch: string;
  speed: number; // ms per tick
  clockMHz: number; // clock frequency in MHz for real execution time
  memorySize: number;
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
  onboardingOpen: boolean;
  /** VS Code–like layout */
  themeMode: ThemeMode;
  diagramPanelOpen: boolean;
  bottomPanelOpen: boolean;
  /** Single navigator for registers / memory / trace */
  sidebarView: PanelId;
  /** Appearance — fonts, density, motion */
  uiFontFamily: UiFontId;
  monoFontFamily: MonoFontId;
  editorFontSize: number;
  uiDensity: UiDensity;
  reducedMotion: boolean;
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
  setOnboardingOpen: (v: boolean) => void;
  setThemeMode: (mode: ThemeMode) => void;
  setDiagramPanelOpen: (v: boolean) => void;
  setBottomPanelOpen: (v: boolean) => void;
  setSidebarView: (id: PanelId) => void;
  setFilePath: (p: string | null) => void;
  addEditorTab: () => void;
  closeEditorTab: (id: string) => void;
  setActiveEditorTab: (id: string) => void;
  renameEditorTab: (id: string, title: string) => void;
  setBreakpoints: (b: number[]) => void;
  toggleBreakpoint: (addr: number) => void;
  setMaxCycleLimit: (n: number | null) => void;
  saveFile: () => Promise<boolean>;
  loadFile: () => Promise<boolean>;
  newFile: () => void;
  setUiFontFamily: (id: UiFontId) => void;
  setMonoFontFamily: (id: MonoFontId) => void;
  setEditorFontSize: (px: number) => void;
  setUiDensity: (d: UiDensity) => void;
  setReducedMotion: (v: boolean) => void;
}

function getActiveSource(state: AppState): string {
  return state.tabBuffers[state.activeEditorTabId] ?? "";
}

export const useStore = create<AppState & AppActions>((set, get) => ({
  editorTabs: [{ id: INITIAL_EDITOR_TAB_ID, title: "Untitled.asm", filePath: null }],
  activeEditorTabId: INITIAL_EDITOR_TAB_ID,
  tabBuffers: { [INITIAL_EDITOR_TAB_ID]: "" },
  arch: "RV32I",
  speed: 100,
  clockMHz: 1000,
  memorySize: 65536,
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
  onboardingOpen: false,
  themeMode: "dark",
  diagramPanelOpen: true,
  bottomPanelOpen: true,
  sidebarView: "registers",
  uiFontFamily: "ibm-plex",
  monoFontFamily: "jetbrains",
  editorFontSize: 13,
  uiDensity: "comfortable",
  reducedMotion: false,

  setSource: (text) =>
    set((state) => ({
      tabBuffers: {
        ...state.tabBuffers,
        [state.activeEditorTabId]: text,
      },
    })),
  setArch: (a) => set({ arch: a }),
  setSpeed: (s) => set({ speed: s }),
  setClockMHz: (m) => set({ clockMHz: m }),
  setMemorySize: (s) => set({ memorySize: s }),
  setSnapshot: (s) => set({ snapshot: s }),
  setToast: (t) => set({ toast: t }),

  assemble: async () => {
    const st = get();
    const source = getActiveSource(st);
    const { arch } = st;
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
    const st = get();
    const { snapshot, arch } = st;
    const source = getActiveSource(st);
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
    const st = get();
    const { arch, memorySize } = st;
    const source = getActiveSource(st);
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
  setOnboardingOpen: (v) => set({ onboardingOpen: v }),
  setThemeMode: (mode) => set({ themeMode: mode }),
  setDiagramPanelOpen: (v) => set({ diagramPanelOpen: v }),
  setBottomPanelOpen: (v) => set({ bottomPanelOpen: v }),
  setSidebarView: (id) => set({ sidebarView: id }),

  setFilePath: (p) =>
    set((state) => ({
      editorTabs: state.editorTabs.map((t) =>
        t.id === state.activeEditorTabId ? { ...t, filePath: p } : t
      ),
    })),

  addEditorTab: () => {
    const id = `tab-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    set((state) => ({
      editorTabs: [
        ...state.editorTabs,
        { id, title: "Untitled.asm", filePath: null },
      ],
      activeEditorTabId: id,
      tabBuffers: { ...state.tabBuffers, [id]: "" },
    }));
  },

  closeEditorTab: (tabId) =>
    set((state) => {
      if (state.editorTabs.length <= 1) return state;
      const nextTabs = state.editorTabs.filter((t) => t.id !== tabId);
      const nextBuffers = { ...state.tabBuffers };
      delete nextBuffers[tabId];
      const nextActive =
        state.activeEditorTabId === tabId
          ? nextTabs[nextTabs.length - 1]!.id
          : state.activeEditorTabId;
      return {
        editorTabs: nextTabs,
        tabBuffers: nextBuffers,
        activeEditorTabId: nextActive,
      };
    }),

  setActiveEditorTab: (id) => set({ activeEditorTabId: id }),

  renameEditorTab: (id, title) =>
    set((state) => ({
      editorTabs: state.editorTabs.map((t) => (t.id === id ? { ...t, title } : t)),
    })),
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
    const st = get();
    const source = getActiveSource(st);
    const { arch, memorySize, breakpoints, speed, maxCycleLimit, panelVisibility, activeEditorTabId } = st;
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
          panel_visibility: panelVisibility,
        },
      });
      const name = path.split(/[/\\]/).pop() ?? "project.asim";
      set((s) => ({
        editorTabs: s.editorTabs.map((t) =>
          t.id === activeEditorTabId ? { ...t, filePath: path, title: name } : t
        ),
      }));
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
      const s = get();
      const id = `tab-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
      const title = path.split(/[/\\]/).pop() ?? "project.asim";
      set({
        editorTabs: [...s.editorTabs, { id, title, filePath: path }],
        activeEditorTabId: id,
        tabBuffers: { ...s.tabBuffers, [id]: data.source },
        arch: data.arch,
        memorySize: data.memory_size ?? 65536,
        breakpoints: data.breakpoints ?? [],
        speed: data.speed ?? 100,
        maxCycleLimit: data.max_cycle_limit ?? null,
        panelVisibility: data.panel_visibility
          ? { ...s.panelVisibility, ...data.panel_visibility }
          : s.panelVisibility,
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
    const id = `tab-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    set((state) => ({
      editorTabs: [...state.editorTabs, { id, title: "Untitled.asm", filePath: null }],
      activeEditorTabId: id,
      tabBuffers: { ...state.tabBuffers, [id]: "" },
      arch: "RV32I",
      breakpoints: [],
      errors: [],
      cycleHistory: [],
    }));
    get().setToast({ message: "New file", type: "info" });
  },

  setUiFontFamily: (id) => set({ uiFontFamily: id }),
  setMonoFontFamily: (id) => set({ monoFontFamily: id }),
  setEditorFontSize: (px) =>
    set({ editorFontSize: Math.min(20, Math.max(11, Math.round(px))) }),
  setUiDensity: (d) => set({ uiDensity: d }),
  setReducedMotion: (v) => set({ reducedMotion: v }),
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
      themeMode: ThemeMode;
      diagramPanelOpen: boolean;
      bottomPanelOpen: boolean;
      uiFontFamily: UiFontId;
      monoFontFamily: MonoFontId;
      editorFontSize: number;
      uiDensity: UiDensity;
      reducedMotion: boolean;
    }>;
    useStore.setState((s) => ({
      panelVisibility: data.panelVisibility
        ? { ...s.panelVisibility, ...data.panelVisibility }
        : s.panelVisibility,
      panelOrder: data.panelOrder ?? s.panelOrder,
      speed: data.speed ?? s.speed,
      memorySize: data.memorySize ?? s.memorySize,
      themeMode: data.themeMode ?? s.themeMode,
      diagramPanelOpen: data.diagramPanelOpen ?? s.diagramPanelOpen,
      bottomPanelOpen: data.bottomPanelOpen ?? s.bottomPanelOpen,
      uiFontFamily: data.uiFontFamily ?? s.uiFontFamily,
      monoFontFamily: data.monoFontFamily ?? s.monoFontFamily,
      editorFontSize: data.editorFontSize ?? s.editorFontSize,
      uiDensity: data.uiDensity ?? s.uiDensity,
      reducedMotion: data.reducedMotion ?? s.reducedMotion,
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
        themeMode: s.themeMode,
        diagramPanelOpen: s.diagramPanelOpen,
        bottomPanelOpen: s.bottomPanelOpen,
        uiFontFamily: s.uiFontFamily,
        monoFontFamily: s.monoFontFamily,
        editorFontSize: s.editorFontSize,
        uiDensity: s.uiDensity,
        reducedMotion: s.reducedMotion,
      })
    );
  } catch {
    // ignore
  }
}
