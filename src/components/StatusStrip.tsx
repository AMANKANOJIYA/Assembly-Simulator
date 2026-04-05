import { useStore } from "../store";
import type { AssemblerError } from "../types";

function fileBaseName(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] ?? path;
}

const ARCH_COLOR: Record<string, string> = {
  RV32I:  "#3b82f6",
  LC3:    "#8b5cf6",
  MIPS:   "#10b981",
  "8085": "#f59e0b",
  "6502": "#ef4444",
  "8086": "#f97316",
};

const RUN_STATE_COLOR: Record<string, string> = {
  IDLE:     "var(--app-fg-muted)",
  RUNNING:  "#34d399",
  PAUSED:   "#fbbf24",
  HALTED:   "#f87171",
  ERROR:    "#f87171",
};

function ErrorBadge({ errors }: { errors: AssemblerError[] }) {
  const setJumpToLineRequest = useStore((s) => s.setJumpToLineRequest);
  if (errors.length === 0) return null;
  const first = errors[0];
  return (
    <button
      type="button"
      className="status-chip status-chip--errors"
      onClick={() => setJumpToLineRequest(first.line)}
      title={`${errors.length} error${errors.length > 1 ? "s" : ""} — click to jump to first error`}
    >
      <span className="status-error-icon" aria-hidden>✕</span>
      {errors.length} error{errors.length > 1 ? "s" : ""}
    </button>
  );
}

/** Highlights execution state: arch, PC, cycles, run mode, I/O wait, halt */
export function StatusStrip() {
  const arch    = useStore((s) => s.arch);
  const errors  = useStore((s) => s.errors);
  const snapshot = useStore((s) => s.snapshot);
  const activeFilePath = useStore((s) => {
    const t = s.editorTabs.find((x) => x.id === s.activeEditorTabId);
    return t?.filePath ?? null;
  });
  const activeTitle = useStore((s) => {
    const t = s.editorTabs.find((x) => x.id === s.activeEditorTabId);
    return t?.title ?? "Untitled";
  });

  const archColor = ARCH_COLOR[arch] ?? "#6b7280";

  const archChip = (
    <span className="status-chip status-chip--arch" title={`Architecture: ${arch}`}>
      <span
        className="status-arch-dot"
        style={{ background: archColor }}
        aria-hidden="true"
      />
      {arch}
    </span>
  );

  if (!snapshot) {
    return (
      <div className="status-strip" role="status">
        {archChip}
        <span className="status-strip-sep" aria-hidden="true" />
        {activeFilePath ? (
          <span className="status-strip-file" title={activeFilePath}>
            {fileBaseName(activeFilePath)}
          </span>
        ) : (
          <span className="status-strip-muted">{activeTitle}</span>
        )}
        <span className="status-strip-muted">Assemble to connect</span>
        <ErrorBadge errors={errors} />
      </div>
    );
  }

  const pc        = snapshot.state.pc;
  const cycles    = snapshot.total_cycles ?? 0;
  const runState  = (snapshot.run_state ?? "IDLE").toUpperCase();
  const halted    = snapshot.halted;
  const ioWait    = !!snapshot.io_input_requested;
  const outLen    = snapshot.io_output?.length ?? 0;
  const stateColor = RUN_STATE_COLOR[runState] ?? "var(--app-fg-muted)";

  return (
    <div className="status-strip" role="status" aria-live="polite">
      {archChip}

      <span className="status-strip-sep" aria-hidden="true" />

      {(activeFilePath || activeTitle) && (
        <span className="status-strip-file" title={activeFilePath ?? activeTitle}>
          {activeFilePath ? fileBaseName(activeFilePath) : activeTitle}
        </span>
      )}

      <span className="status-chip status-chip--mono" title="Program counter">
        PC <strong>0x{pc.toString(16).toUpperCase().padStart(8, "0")}</strong>
      </span>

      <span className="status-chip status-chip--mono" title="Total cycles executed">
        {cycles.toLocaleString()} cycles
      </span>

      <span
        className="status-chip status-chip--state"
        style={{ color: stateColor, borderColor: `color-mix(in srgb, ${stateColor} 35%, transparent)` }}
        aria-label={`Run state: ${runState}`}
      >
        <span
          className={`status-state-dot ${runState === "RUNNING" ? "status-state-dot--pulse" : ""}`}
          style={{ background: stateColor }}
          aria-hidden="true"
        />
        {runState}
      </span>

      {halted && (
        <span className="status-chip status-chip--halt" aria-label="CPU halted">
          HALTED
        </span>
      )}
      {ioWait && (
        <span className="status-chip status-chip--io" title="Waiting for trap/syscall input">
          INPUT WAIT
        </span>
      )}
      {outLen > 0 && (
        <span className="status-chip status-chip--out" title="Program has written output">
          {outLen} chars out
        </span>
      )}
      <ErrorBadge errors={errors} />
    </div>
  );
}
