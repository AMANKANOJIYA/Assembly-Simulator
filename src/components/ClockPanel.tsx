import { useState, useEffect, useCallback } from "react";
import { useStore } from "../store";
import { decodeRv32i } from "../utils/decodeInstr";

const PULSE_WIDTH = 20;
const GAP = 8;
const PAD = 20;
const WAVE_LOW_Y = 36;
const WAVE_HIGH_Y = 8;
const CYCLE_WIDTH = PULSE_WIDTH + GAP;
// Cap displayed history to prevent rendering thousands of SVG/table nodes
const MAX_DISPLAY_HISTORY = 500;

function formatTimeNs(ns: number): string {
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
  if (ns >= 1e3) return `${(ns / 1e3).toFixed(2)} µs`;
  return `${ns.toFixed(0)} ns`;
}

function stageToClass(stage: string): string {
  return stage.toLowerCase().replace(/\s+/g, "-");
}

export function ClockPanel() {
  const [selectedCycle, setSelectedCycle] = useState<number | null>(null);
  const snapshot = useStore((s) => s.snapshot);
  const speed = useStore((s) => s.speed);
  const clockMHz = useStore((s) => s.clockMHz);
  const cycleHistory = useStore((s) => s.cycleHistory);
  const cycleGraphOpen = useStore((s) => s.cycleGraphOpen);
  const setCycleHistoryGraphOpen = useStore((s) => s.setCycleHistoryGraphOpen);
  const setClockMHz = useStore((s) => s.setClockMHz);

  const cycles = snapshot?.total_cycles ?? 0;
  const runState = snapshot?.run_state ?? "IDLE";

  // Only render the most recent MAX_DISPLAY_HISTORY entries to keep the UI responsive
  const history = cycleHistory.length > MAX_DISPLAY_HISTORY
    ? cycleHistory.slice(-MAX_DISPLAY_HISTORY)
    : cycleHistory;
  const historyTruncated = cycleHistory.length > MAX_DISPLAY_HISTORY;
  const totalWidth = history.length ? history.length * CYCLE_WIDTH : 100;
  const timePerCycleNs = 1000 / clockMHz;

  useEffect(() => {
    if (!cycleGraphOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setCycleHistoryGraphOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cycleGraphOpen, setCycleHistoryGraphOpen]);

  const exportCsv = useCallback(() => {
    if (history.length === 0) return;
    const tpc = 1000 / clockMHz;
    const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
    const lines = [
      ["cycle", "stage", "instruction_bits", "action", "time_ns_per_cycle"].join(","),
      ...history.map((h) =>
        [
          h.cycle,
          esc(h.stage),
          h.instructionBits ?? "",
          esc(h.action || ""),
          String(tpc),
        ].join(",")
      ),
    ];
    const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `cycle-timing-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [history, clockMHz]);

  return (
    <>
      <div className="clock-panel">
        <div className="clock-items-left">
          <div className="clock-item">
            <span className="clock-label">Cycles</span>
            <span className="clock-value">{cycles}</span>
          </div>
          <div className="clock-item">
            <span className="clock-label">State</span>
            <span className={`clock-value state-${runState.toLowerCase()}`}>{runState}</span>
          </div>
        </div>
        <button
          type="button"
          className="clock-graph-trigger"
          onClick={() => setCycleHistoryGraphOpen(!cycleGraphOpen)}
          title={cycleGraphOpen ? "Close cycle timing graph (Esc)" : "Open cycle timing graph"}
          aria-expanded={cycleGraphOpen}
        >
          <span className="clock-graph-icon" aria-hidden>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75">
              <path d="M4 14.5V19a1 1 0 0 0 1 1h5M4 9.5V5a1 1 0 0 1 1-1h5M16 4h4a1 1 0 0 1 1 1v4M16 20h4a1 1 0 0 0 1-1v-4" />
              <path d="M9 9h6v6H9z" />
            </svg>
          </span>
          <span className="clock-graph-cycle">{cycles}</span>
          <span className="clock-graph-arrow" aria-hidden>
            {cycleGraphOpen ? "▼" : "▲"}
          </span>
        </button>
        <div className="clock-items-right">
          <div className="clock-item">
            <span className="clock-label">Mode</span>
            <span className="clock-value">Instruction</span>
          </div>
          <div className="clock-item">
            <span className="clock-label">Speed</span>
            <span className="clock-value">{speed}ms</span>
          </div>
        </div>
      </div>

      {cycleGraphOpen && (
        <div
          className="cycle-graph-overlay"
          onClick={() => setCycleHistoryGraphOpen(false)}
          role="presentation"
        >
          <div
            className="cycle-graph-popup"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-label="Cycle timing graph"
          >
            <header className="cycle-graph-header">
              <div className="cycle-graph-title-block">
                <h2 className="cycle-graph-title">Cycle Timing Graph</h2>
                <p className="cycle-graph-subtitle">Pipeline stages, square wave per cycle, real time from clock (MHz)</p>
              </div>
              <div className="cycle-graph-header-actions">
                <label className="cycle-clock-input-wrap">
                  <span className="cycle-clock-label">Clock (MHz)</span>
                  <input
                    type="number"
                    min={1}
                    max={10000}
                    value={clockMHz}
                    onChange={(e) => {
                      const v = parseInt(e.target.value, 10);
                      if (!isNaN(v) && v >= 1) setClockMHz(v);
                    }}
                    className="cycle-clock-input"
                  />
                </label>
                {selectedCycle != null && (
                  <button type="button" className="btn btn-small cycle-graph-clear-sel" onClick={() => setSelectedCycle(null)}>
                    Clear selection
                  </button>
                )}
                {history.length > 0 && (
                  <button type="button" className="btn btn-small" onClick={exportCsv} title="Download table as CSV">
                    Export CSV
                  </button>
                )}
                <button
                  type="button"
                  className="cycle-graph-close"
                  onClick={() => setCycleHistoryGraphOpen(false)}
                  aria-label="Close"
                  title="Close (Esc)"
                >
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M18 6 6 18M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </header>

            <div className="cycle-graph-body">
              {history.length === 0 ? (
                <div className="cycle-graph-empty">
                  <div className="cycle-graph-empty-icon" aria-hidden>
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.25">
                      <path d="M4 14.5V19a1 1 0 0 0 1 1h5M4 9.5V5a1 1 0 0 1 1-1h5M16 4h4a1 1 0 0 1 1 1v4M16 20h4a1 1 0 0 0 1-1v-4" />
                      <path d="M9 9h6v6H9z" />
                    </svg>
                  </div>
                  <p className="cycle-graph-empty-title">No cycle data yet</p>
                  <p className="cycle-graph-empty-text">Assemble, then <strong>Step</strong> or <strong>Run</strong> to record pipeline stages. Each pulse is one cycle.</p>
                </div>
              ) : (
                <div className="cycle-wave-container">
                  <div className="cycle-graph-meta">
                    <span className="cycle-graph-meta-chip">
                      Clock <strong>{clockMHz} MHz</strong>
                      <span className="cycle-graph-meta-sep">·</span>
                      {formatTimeNs(timePerCycleNs)} / cycle
                    </span>
                    <span className="cycle-graph-meta-chip">
                      Recorded <strong>{history.length}</strong> steps
                      <span className="cycle-graph-meta-sep">·</span>
                      Sim total {cycles} cycles ≈ {formatTimeNs(cycles * timePerCycleNs)}
                    </span>
                  </div>

                  <div className="cycle-wave-toolbar">
                    <span className="cycle-graph-hint-inline">Click a pulse or table row to highlight. Esc closes.</span>
                    {historyTruncated && (
                      <span className="cycle-graph-hint-inline cycle-graph-hint-truncated">
                        Showing last {MAX_DISPLAY_HISTORY} of {cycleHistory.length} steps. Export CSV for full data.
                      </span>
                    )}
                  </div>

                  <div className="cycle-wave-scroll">
                    <svg
                      className="cycle-wave-svg"
                      viewBox={`0 0 ${Math.max(totalWidth + PAD * 2, 320)} 60`}
                      preserveAspectRatio="xMinYMid meet"
                      style={{ minWidth: Math.max(totalWidth + PAD * 2, 320), height: 60 }}
                      aria-label="Cycle square wave"
                    >
                      {history.map(({ cycle }, i) => {
                        const x0 = PAD + i * CYCLE_WIDTH;
                        const x1 = x0 + PULSE_WIDTH;
                        const isSelected = selectedCycle === cycle;
                        const pulsePath = `M ${x0} ${WAVE_LOW_Y} L ${x0} ${WAVE_HIGH_Y} L ${x1} ${WAVE_HIGH_Y} L ${x1} ${WAVE_LOW_Y}`;
                        return (
                          <g key={cycle}>
                            <path
                              d={pulsePath}
                              fill="none"
                              className={`cycle-wave-pulse${isSelected ? " cycle-wave-pulse--selected" : ""}`}
                              strokeWidth={isSelected ? 3 : 2}
                              strokeLinecap="square"
                              strokeLinejoin="miter"
                              style={{ cursor: "pointer" }}
                              onClick={(e) => {
                                e.stopPropagation();
                                setSelectedCycle(selectedCycle === cycle ? null : cycle);
                              }}
                            />
                            <text
                              x={x0 + PULSE_WIDTH / 2}
                              y={52}
                              textAnchor="middle"
                              className={`cycle-label${isSelected ? " selected" : ""}`}
                            >
                              {cycle}
                            </text>
                          </g>
                        );
                      })}
                    </svg>
                  </div>

                  <div className="cycle-table-wrap">
                    <table className="cycle-table">
                      <thead>
                        <tr>
                          <th>Cycle</th>
                          <th>Stage</th>
                          <th>Instruction</th>
                          <th>What happens</th>
                          <th>Time</th>
                        </tr>
                      </thead>
                      <tbody>
                        {history.map(({ cycle, stage, instructionBits, action }) => {
                          const decoded = instructionBits != null ? decodeRv32i(instructionBits) : null;
                          const isSelected = selectedCycle === cycle;
                          return (
                            <tr
                              key={cycle}
                              className={isSelected ? "selected" : ""}
                              onClick={() => setSelectedCycle(selectedCycle === cycle ? null : cycle)}
                            >
                              <td>
                                <code className="cycle-table-cycle">C{cycle}</code>
                              </td>
                              <td>
                                <span className={`stage-badge stage-${stageToClass(stage)}`}>{stage}</span>
                              </td>
                              <td>
                                {decoded ? <span className="instr-mnemonic">{decoded.mnemonic}</span> : "—"}
                              </td>
                              <td className="cycle-action-cell">{action || "—"}</td>
                              <td className="cycle-table-time">{formatTimeNs(timePerCycleNs)}</td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>

                  <footer className="cycle-graph-footer-hint">
                    5-stage pipeline: Fetch → Decode → Execute → Memory → Write-back. Clock (MHz) maps cycles to wall-clock time.
                  </footer>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
