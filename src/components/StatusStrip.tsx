import { useStore } from "../store";

/** Highlights execution state: PC, run mode, I/O wait, halt */
export function StatusStrip() {
  const snapshot = useStore((s) => s.snapshot);
  const activeFilePath = useStore((s) => {
    const t = s.editorTabs.find((x) => x.id === s.activeEditorTabId);
    return t?.filePath ?? null;
  });
  const activeTitle = useStore((s) => {
    const t = s.editorTabs.find((x) => x.id === s.activeEditorTabId);
    return t?.title ?? "Untitled";
  });

  if (!snapshot) {
    return (
      <div className="status-strip" role="status">
        {activeFilePath && (
          <span className="status-strip-file" title={activeFilePath}>
            {activeFilePath.split(/[/\\]/).pop()}
          </span>
        )}
        {!activeFilePath && <span className="status-strip-muted">{activeTitle}</span>}
        <span className="status-strip-muted">No simulator state — assemble to connect</span>
      </div>
    );
  }

  const pc = snapshot.state.pc;
  const runState = snapshot.run_state ?? "IDLE";
  const halted = snapshot.halted;
  const ioWait = !!snapshot.io_input_requested;
  const outLen = snapshot.io_output?.length ?? 0;

  return (
    <div className="status-strip" role="status" aria-live="polite">
      {activeFilePath || activeTitle ? (
        <span className="status-strip-file" title={activeFilePath ?? activeTitle}>
          {activeFilePath ? activeFilePath.split(/[/\\]/).pop() : activeTitle}
        </span>
      ) : null}
      <span className="status-chip status-chip--mono" title="Program counter">
        PC <strong>0x{pc.toString(16).toUpperCase().padStart(8, "0")}</strong>
      </span>
      <span className={`status-chip status-chip--state state-${String(runState).toLowerCase()}`}>
        {runState}
      </span>
      {halted && <span className="status-chip status-chip--halt">HALTED</span>}
      {ioWait && (
        <span className="status-chip status-chip--io" title="Waiting for trap/syscall input">
          INPUT WAIT
        </span>
      )}
      {outLen > 0 && (
        <span className="status-chip status-chip--out" title="Program has written output">
          OUTPUT ({outLen} chars)
        </span>
      )}
    </div>
  );
}
