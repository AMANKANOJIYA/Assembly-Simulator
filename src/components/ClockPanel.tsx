import { useState } from "react";
import { useStore } from "../store";
import { decodeRv32i } from "../utils/decodeInstr";

const PULSE_WIDTH = 20;
const GAP = 8;
const PAD = 20;
const WAVE_LOW_Y = 36;
const WAVE_HIGH_Y = 8;
const CYCLE_WIDTH = PULSE_WIDTH + GAP;

function formatTimeNs(ns: number): string {
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
  if (ns >= 1e3) return `${(ns / 1e3).toFixed(2)} µs`;
  return `${ns.toFixed(0)} ns`;
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

  const history = cycleHistory;
  const totalWidth = history.length ? history.length * CYCLE_WIDTH : 100;
  const timePerCycleNs = 1000 / clockMHz; // 1 cycle at clockMHz = timePerCycleNs ns

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
            <span className={`clock-value state-${runState.toLowerCase()}`}>
              {runState}
            </span>
          </div>
        </div>
        <button
          type="button"
          className="clock-graph-trigger"
          onClick={() => setCycleHistoryGraphOpen(!cycleGraphOpen)}
          title={cycleGraphOpen ? "Close cycle timing graph" : "Open cycle timing graph"}
          aria-expanded={cycleGraphOpen}
        >
          <span className="clock-graph-icon" aria-hidden>⊞</span>
          <span className="clock-graph-cycle">{cycles}</span>
          <span className="clock-graph-arrow">{cycleGraphOpen ? "▼" : "▲"}</span>
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
            aria-label="Cycle timing graph"
            style={{ width: "100%" }}
          >
            <div className="cycle-graph-header">
              <h3>Cycle Timing Graph</h3>
              <div className="cycle-graph-header-actions">
                <label className="cycle-clock-input-wrap">
                  <span>Clock (MHz):</span>
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
                <button
                  type="button"
                  className="btn btn-small"
                  onClick={() => setCycleHistoryGraphOpen(false)}
                >
                  Close
                </button>
              </div>
            </div>
            <div className="cycle-graph-body">
              {history.length === 0 ? (
                <div className="cycle-graph-empty">
                  Run or step the program to see cycle timing
                </div>
              ) : (
                <div className="cycle-wave-container">
                  <div className="cycle-graph-meta">
                    <span>
                      Clock: <strong>{clockMHz} MHz</strong> →{" "}
                      {formatTimeNs(timePerCycleNs)} per cycle
                    </span>
                    <span>
                      Total: {cycles} cycles ={" "}
                      {formatTimeNs(cycles * timePerCycleNs)}
                    </span>
                  </div>
                  <div className="cycle-wave-scroll">
                    <svg
                      className="cycle-wave-svg"
                      viewBox={`0 0 ${Math.max(totalWidth + PAD * 2, 320)} 60`}
                      preserveAspectRatio="xMinYMid meet"
                      style={{ minWidth: Math.max(totalWidth + PAD * 2, 320), height: 60 }}
                    >
                    {/* Square wave - draw each pulse separately for hover/select */}
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
                            stroke={isSelected ? "#fbbf24" : "#7dd3fc"}
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
                              <td>C{cycle}</td>
                              <td>
                                <span className={`stage-badge stage-${stage.toLowerCase().replace(/\s+/g, "-")}`}>
                                  {stage}
                                </span>
                              </td>
                              <td>
                                {decoded ? (
                                  <span className="instr-mnemonic">{decoded.mnemonic}</span>
                                ) : (
                                  "—"
                                )}
                              </td>
                              <td className="cycle-action-cell">{action || "—"}</td>
                              <td>{formatTimeNs(timePerCycleNs)}</td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                  <div className="cycle-graph-clock-hint">
                    5-stage pipeline (Fetch → Decode → Execute → Memory → Write-back). Set clock (MHz) to compute real execution time.
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
