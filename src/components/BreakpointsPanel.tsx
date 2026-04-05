import { useStore } from "../store";

export function BreakpointsPanel() {
  const breakpoints       = useStore((s) => s.breakpoints);
  const snapshot          = useStore((s) => s.snapshot);
  const toggleBreakpoint  = useStore((s) => s.toggleBreakpoint);
  const setJumpToLineRequest = useStore((s) => s.setJumpToLineRequest);

  const sourceMap = snapshot?.source_map ?? [];

  /** Given a PC address, find the source line number */
  const pcToLine = (pc: number): number | null => {
    const entry = sourceMap.find((e) => e.pc === pc);
    return entry?.line ?? null;
  };

  const handleJump = (pc: number) => {
    const line = pcToLine(pc);
    if (line != null) setJumpToLineRequest(line);
  };

  return (
    <div className="panel breakpoints-panel">
      <div className="panel-header">
        <h3 className="panel-title">Breakpoints</h3>
        {breakpoints.length > 0 && (
          <span className="panel-badge">{breakpoints.length}</span>
        )}
      </div>
      <div className="breakpoints-body">
        {breakpoints.length === 0 ? (
          <div className="breakpoints-empty">
            <span className="breakpoints-empty-icon">○</span>
            <p>No breakpoints set</p>
            <p className="breakpoints-empty-hint">Click the gutter margin in the editor to add a breakpoint</p>
          </div>
        ) : (
          <ul className="breakpoints-list">
            {breakpoints.map((pc) => {
              const line = pcToLine(pc);
              const isCurrentPc = snapshot?.state.pc === pc;
              return (
                <li
                  key={pc}
                  className={`breakpoint-item${isCurrentPc ? " breakpoint-item--active" : ""}`}
                >
                  <span className="breakpoint-dot" aria-hidden />
                  <div className="breakpoint-info">
                    <button
                      type="button"
                      className="breakpoint-jump-btn"
                      onClick={() => handleJump(pc)}
                      disabled={line == null}
                      title={line != null ? `Jump to line ${line}` : "Source map unavailable (assemble first)"}
                    >
                      {line != null ? `Line ${line}` : "?"}
                    </button>
                    <span className="breakpoint-addr">PC 0x{pc.toString(16).toUpperCase().padStart(8, "0")}</span>
                  </div>
                  <button
                    type="button"
                    className="breakpoint-remove-btn"
                    onClick={() => toggleBreakpoint(pc)}
                    title="Remove breakpoint"
                    aria-label={`Remove breakpoint at PC 0x${pc.toString(16)}`}
                  >
                    ✕
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
