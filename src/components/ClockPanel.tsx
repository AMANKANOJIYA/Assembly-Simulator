import { useState, useRef, useEffect, useMemo, useCallback } from "react";
import { useStore } from "../store";
import { decodeRv32i } from "../utils/decodeInstr";

// ── Constants ────────────────────────────────────────────────────────────────

const STAGE_ORDER = ["Fetch", "Decode", "Execute", "Memory", "Write-back", "Halted"];

const STAGE_COLORS: Record<string, string> = {
  "Fetch":      "#6366f1",
  "Decode":     "#22c55e",
  "Execute":    "#f59e0b",
  "Memory":     "#ec4899",
  "Write-back": "#3b82f6",
  "Halted":     "#ef4444",
};

// Per-instruction-instance color palette (12 vivid, cycles through)
const INSTR_PALETTE = [
  "#6366f1","#f59e0b","#22c55e","#ec4899","#3b82f6","#ef4444",
  "#14b8a6","#8b5cf6","#f97316","#06b6d4","#84cc16","#a855f7",
];

const LANE_H   = 30;   // px per stage row
const LANE_GAP = 3;
const LANE_STEP = LANE_H + LANE_GAP;
// LABEL_W (80px) matches .gantt-label-col width in CSS
const AXIS_H   = 22;   // time axis at bottom
const BASE_CELL_W = 36; // px per cycle at zoom = 1
const MAX_DISPLAY = 500;

// ── Helpers ──────────────────────────────────────────────────────────────────

function fmtNs(ns: number): string {
  if (ns >= 1e9) return `${(ns / 1e9).toFixed(3)} s`;
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
  if (ns >= 1e3) return `${(ns / 1e3).toFixed(2)} µs`;
  return `${ns.toFixed(1)} ns`;
}

type Entry = { cycle: number; stage: string; instructionBits?: number; action: string };

function getMnemonic(bits: number | undefined, arch: string): string {
  if (bits === undefined) return "—";
  if (arch === "RV32I") {
    const d = decodeRv32i(bits);
    if (d?.mnemonic) return d.mnemonic;
  }
  return `0x${bits.toString(16).padStart(8, "0")}`;
}

interface InstrGroup {
  id: number;            // sequential instance id
  bits?: number;
  mnemonic: string;
  startCycle: number;
  endCycle: number;
  stageCount: number;    // number of pipeline stages
  entries: Entry[];
}

/** Group consecutive entries with the same instructionBits into instruction instances */
function groupByInstruction(history: Entry[], arch: string): InstrGroup[] {
  const out: InstrGroup[] = [];
  let id = 0;
  let cur: InstrGroup | null = null;

  for (const e of history) {
    if (!cur || e.instructionBits !== cur.bits) {
      if (cur) out.push(cur);
      id++;
      cur = {
        id, bits: e.instructionBits,
        mnemonic: getMnemonic(e.instructionBits, arch),
        startCycle: e.cycle, endCycle: e.cycle,
        stageCount: 0, entries: [],
      };
    }
    cur.entries.push(e);
    cur.endCycle = e.cycle;
    cur.stageCount++;
  }
  if (cur) out.push(cur);
  return out;
}

// ── Tooltip ──────────────────────────────────────────────────────────────────

interface TooltipState {
  x: number; y: number;
  entry: Entry;
  mn: string;
  groupId: number;
  timeNs: number;
}

// ── Gantt Chart ──────────────────────────────────────────────────────────────

function GanttChart({
  history,
  clockMHz,
  arch,
  hiddenStages,
}: {
  history: Entry[];
  clockMHz: number;
  arch: string;
  hiddenStages: Set<string>;
}) {
  const [zoom, setZoom] = useState(1);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const [selected, setSelected] = useState<number | null>(null); // cycle number
  const containerRef = useRef<HTMLDivElement>(null);
  const timePerCycleNs = 1000 / clockMHz;
  const cellW = Math.max(12, Math.round(BASE_CELL_W * zoom));

  // Wheel zoom
  const handleWheel = useCallback((e: WheelEvent) => {
    e.preventDefault();
    setZoom((z) => Math.min(6, Math.max(0.3, z * (e.deltaY < 0 ? 1.15 : 0.87))));
  }, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener("wheel", handleWheel, { passive: false });
    return () => el.removeEventListener("wheel", handleWheel);
  }, [handleWheel]);

  // Auto-scroll right on new data
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollLeft = containerRef.current.scrollWidth;
    }
  }, [history.length]);

  const groups = useMemo(() => groupByInstruction(history, arch), [history, arch]);
  // Map cycle → groupId for fast lookup
  const cycleGroupMap = useMemo(() => {
    const m = new Map<number, number>();
    for (const g of groups) for (const e of g.entries) m.set(e.cycle, g.id);
    return m;
  }, [groups]);

  const visibleStages = STAGE_ORDER.filter((s) => {
    const inData = history.some((e) => e.stage === s);
    return inData && !hiddenStages.has(s);
  });

  if (history.length === 0) return null;
  const minCycle = history[0].cycle;
  const maxCycle = history[history.length - 1].cycle;
  const totalCols = maxCycle - minCycle + 1;

  const svgW = totalCols * cellW;
  const svgH = visibleStages.length * LANE_STEP + AXIS_H;

  // Tick density: aim for ~60px apart
  const tickEvery = Math.max(1, Math.round(60 / cellW));

  return (
    <div className="gantt-wrap">
      {/* Stage label column (sticky left) */}
      <div className="gantt-label-col" style={{ minHeight: svgH }}>
        {visibleStages.map((s, i) => (
          <div
            key={s}
            className="gantt-stage-label"
            style={{
              top: i * LANE_STEP,
              height: LANE_H,
              color: STAGE_COLORS[s] ?? "var(--app-fg-muted)",
            }}
          >
            {s}
          </div>
        ))}
        <div className="gantt-axis-spacer" style={{ height: AXIS_H }} />
      </div>

      {/* Scrollable chart */}
      <div className="gantt-scroll" ref={containerRef}>
        <svg
          width={svgW}
          height={svgH}
          className="gantt-svg"
          onMouseLeave={() => setTooltip(null)}
        >
          {/* Lane backgrounds */}
          {visibleStages.map((s, i) => (
            <rect
              key={s}
              x={0} y={i * LANE_STEP}
              width={svgW} height={LANE_H}
              fill={i % 2 === 0 ? "var(--gantt-row-even)" : "var(--gantt-row-odd)"}
            />
          ))}

          {/* Vertical cycle grid lines */}
          {Array.from({ length: totalCols }, (_, idx) => {
            const c = minCycle + idx;
            if ((c - minCycle) % tickEvery !== 0) return null;
            const x = idx * cellW;
            return (
              <line key={c} x1={x} y1={0} x2={x} y2={svgH - AXIS_H}
                stroke="var(--gantt-grid)" strokeWidth={1} />
            );
          })}

          {/* Stage blocks */}
          {history.map((e) => {
            const si = visibleStages.indexOf(e.stage);
            if (si === -1) return null;
            const gid = cycleGroupMap.get(e.cycle) ?? 0;
            const color = INSTR_PALETTE[(gid - 1) % INSTR_PALETTE.length];
            const x = (e.cycle - minCycle) * cellW + 1;
            const y = si * LANE_STEP + 1;
            const w = Math.max(4, cellW - 2);
            const h = LANE_H - 2;
            const isSelected = selected === e.cycle;
            const mn = getMnemonic(e.instructionBits, arch);

            return (
              <g key={e.cycle + e.stage}>
                <rect
                  x={x} y={y} width={w} height={h}
                  rx={3}
                  fill={color}
                  opacity={isSelected ? 1 : 0.82}
                  stroke={isSelected ? "#fff" : "transparent"}
                  strokeWidth={1.5}
                  style={{ cursor: "pointer" }}
                  onClick={() => setSelected(isSelected ? null : e.cycle)}
                  onMouseEnter={(ev) => setTooltip({
                    x: ev.clientX + 12, y: ev.clientY - 8,
                    entry: e, mn, groupId: gid,
                    timeNs: (e.cycle - minCycle) * timePerCycleNs,
                  })}
                  onMouseLeave={() => setTooltip(null)}
                />
                {/* Mnemonic label inside block if wide enough */}
                {w >= 40 && (
                  <text
                    x={x + w / 2} y={y + h / 2 + 4}
                    textAnchor="middle"
                    fill="#fff"
                    fontSize={Math.min(10, cellW / 3.5)}
                    fontFamily="monospace"
                    pointerEvents="none"
                    style={{ userSelect: "none" }}
                  >
                    {mn.length > 8 ? mn.slice(0, 7) + "…" : mn}
                  </text>
                )}
              </g>
            );
          })}

          {/* Time axis */}
          {Array.from({ length: totalCols }, (_, idx) => {
            const c = minCycle + idx;
            if ((c - minCycle) % tickEvery !== 0) return null;
            const x = idx * cellW;
            const t = idx * timePerCycleNs;
            return (
              <g key={`axis-${c}`}>
                <line x1={x} y1={svgH - AXIS_H} x2={x} y2={svgH - AXIS_H + 4}
                  stroke="var(--gantt-grid)" strokeWidth={1} />
                <text
                  x={x + 3} y={svgH - 4}
                  fill="var(--gantt-axis-text)"
                  fontSize={9} fontFamily="monospace"
                >
                  {fmtNs(t)}
                </text>
              </g>
            );
          })}
        </svg>
      </div>

      {/* Tooltip */}
      {tooltip && (
        <div
          className="gantt-tooltip"
          style={{ left: tooltip.x, top: tooltip.y }}
        >
          <div className="gantt-tooltip-row">
            <span className="gantt-tooltip-key">Cycle</span>
            <span className="gantt-tooltip-val">#{tooltip.entry.cycle}</span>
          </div>
          <div className="gantt-tooltip-row">
            <span className="gantt-tooltip-key">Stage</span>
            <span className="gantt-tooltip-val" style={{ color: STAGE_COLORS[tooltip.entry.stage] }}>
              {tooltip.entry.stage}
            </span>
          </div>
          <div className="gantt-tooltip-row">
            <span className="gantt-tooltip-key">Instr</span>
            <span className="gantt-tooltip-val">{tooltip.mn}</span>
          </div>
          <div className="gantt-tooltip-row">
            <span className="gantt-tooltip-key">Time</span>
            <span className="gantt-tooltip-val">{fmtNs(tooltip.timeNs)}</span>
          </div>
          {tooltip.entry.action && (
            <div className="gantt-tooltip-action">{tooltip.entry.action}</div>
          )}
        </div>
      )}

      {/* Zoom hint */}
      <div className="gantt-zoom-hint">
        <span>Scroll to zoom · Drag to pan</span>
        <span className="gantt-zoom-val">{Math.round(zoom * 100)}%</span>
        <button className="gantt-zoom-btn" onClick={() => setZoom(1)}>Reset</button>
      </div>
    </div>
  );
}

// ── Stats Bar ────────────────────────────────────────────────────────────────

function StatsBar({
  history,
  clockMHz,
  arch,
}: {
  history: Entry[];
  clockMHz: number;
  arch: string;
}) {
  const timePerCycleNs = 1000 / clockMHz;

  const { instrCount, cpi, simIps, totalTimeNs, stageBreakdown } = useMemo(() => {
    if (history.length === 0) return { instrCount: 0, cpi: 0, simIps: 0, totalTimeNs: 0, stageBreakdown: {} };
    const groups = groupByInstruction(history, arch);
    const instrCount = groups.length;
    const totalCycles = history.length;
    const cpi = totalCycles / Math.max(1, instrCount);
    const simIps = instrCount === 0 ? 0 : (clockMHz * 1e6) / cpi;
    const totalTimeNs = history.length * timePerCycleNs;
    const stageBreakdown: Record<string, number> = {};
    for (const e of history) {
      stageBreakdown[e.stage] = (stageBreakdown[e.stage] ?? 0) + 1;
    }
    return { instrCount, cpi, simIps, totalTimeNs, stageBreakdown };
  }, [history, arch, clockMHz, timePerCycleNs]);

  const statFmt = (n: number) =>
    n >= 1e9 ? `${(n / 1e9).toFixed(2)}G` :
    n >= 1e6 ? `${(n / 1e6).toFixed(2)}M` :
    n >= 1e3 ? `${(n / 1e3).toFixed(1)}K` : n.toFixed(0);

  return (
    <div className="gantt-stats-bar">
      <div className="gantt-stat">
        <span className="gantt-stat-label">Cycles recorded</span>
        <span className="gantt-stat-value">{history.length}</span>
      </div>
      <div className="gantt-stat">
        <span className="gantt-stat-label">Instructions</span>
        <span className="gantt-stat-value">{instrCount}</span>
      </div>
      <div className="gantt-stat">
        <span className="gantt-stat-label">CPI</span>
        <span className="gantt-stat-value">{cpi.toFixed(2)}</span>
      </div>
      <div className="gantt-stat">
        <span className="gantt-stat-label">Sim IPS</span>
        <span className="gantt-stat-value">{statFmt(simIps)}</span>
      </div>
      <div className="gantt-stat">
        <span className="gantt-stat-label">Total sim time</span>
        <span className="gantt-stat-value">{fmtNs(totalTimeNs)}</span>
      </div>
      {/* Stage breakdown */}
      <div className="gantt-stage-bar-wrap" title="Stage breakdown">
        {Object.entries(stageBreakdown).map(([s, n]) => (
          <div
            key={s}
            className="gantt-stage-bar-seg"
            style={{
              width: `${(n / Math.max(1, history.length)) * 100}%`,
              background: STAGE_COLORS[s] ?? "#6b7280",
            }}
            title={`${s}: ${n} cycles (${((n / Math.max(1, history.length)) * 100).toFixed(1)}%)`}
          />
        ))}
      </div>
    </div>
  );
}

// ── Instruction Summary Table ────────────────────────────────────────────────

function InstrSummaryTable({
  history,
  clockMHz,
  arch,
}: {
  history: Entry[];
  clockMHz: number;
  arch: string;
}) {
  const timePerCycleNs = 1000 / clockMHz;

  const rows = useMemo(() => {
    const groups = groupByInstruction(history, arch);
    const map = new Map<string, { count: number; totalCycles: number }>();
    for (const g of groups) {
      const cur = map.get(g.mnemonic) ?? { count: 0, totalCycles: 0 };
      map.set(g.mnemonic, { count: cur.count + 1, totalCycles: cur.totalCycles + g.stageCount });
    }
    const total = history.length;
    return Array.from(map.entries())
      .map(([mn, { count, totalCycles }]) => ({
        mnemonic: mn,
        count,
        totalCycles,
        avgCycles: totalCycles / Math.max(1, count),
        avgTimeNs: (totalCycles / Math.max(1, count)) * timePerCycleNs,
        pct: (totalCycles / Math.max(1, total)) * 100,
      }))
      .sort((a, b) => b.totalCycles - a.totalCycles);
  }, [history, arch, timePerCycleNs]);

  if (rows.length === 0) return null;

  return (
    <div className="gantt-summary-wrap">
      <h4 className="gantt-section-title">Instruction Timing Summary</h4>
      <div className="cycle-table-wrap">
        <table className="cycle-table">
          <thead>
            <tr>
              <th>Instruction</th>
              <th>Count</th>
              <th>Total cycles</th>
              <th>Avg cycles</th>
              <th>Avg time / instr</th>
              <th>% of execution</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.mnemonic}>
                <td><span className="instr-mnemonic">{r.mnemonic}</span></td>
                <td>{r.count}</td>
                <td>{r.totalCycles}</td>
                <td>{r.avgCycles.toFixed(1)}</td>
                <td className="cycle-table-time">{fmtNs(r.avgTimeNs)}</td>
                <td>
                  <div className="gantt-pct-bar-wrap">
                    <div className="gantt-pct-bar" style={{ width: `${r.pct}%` }} />
                    <span className="gantt-pct-label">{r.pct.toFixed(1)}%</span>
                  </div>
                </td>
                <td className="cycle-table-time">{fmtNs(r.avgTimeNs * r.count)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ── Raw Cycles Table (collapsible) ───────────────────────────────────────────

function RawCyclesTable({
  history,
  clockMHz,
  arch,
  minCycle,
}: {
  history: Entry[];
  clockMHz: number;
  arch: string;
  minCycle: number;
}) {
  const [open, setOpen] = useState(false);
  const timePerCycleNs = 1000 / clockMHz;

  return (
    <div className="gantt-raw-section">
      <button
        type="button"
        className="gantt-collapse-btn"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
      >
        <span className="gantt-collapse-chevron">{open ? "▼" : "▶"}</span>
        Raw cycle log ({history.length} entries)
      </button>
      {open && (
        <div className="cycle-table-wrap">
          <table className="cycle-table">
            <thead>
              <tr>
                <th>Cycle</th>
                <th>Stage</th>
                <th>Instruction</th>
                <th>Time offset</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              {history.map((e) => {
                const mn = getMnemonic(e.instructionBits, arch);
                const t = (e.cycle - minCycle) * timePerCycleNs;
                return (
                  <tr key={e.cycle + e.stage}>
                    <td><code className="cycle-table-cycle">C{e.cycle}</code></td>
                    <td>
                      <span
                        className={`stage-badge stage-${e.stage.toLowerCase().replace(/\s+/g, "-")}`}
                      >
                        {e.stage}
                      </span>
                    </td>
                    <td><span className="instr-mnemonic">{mn}</span></td>
                    <td className="cycle-table-time">{fmtNs(t)}</td>
                    <td className="cycle-action-cell">{e.action || "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ── Full popup ───────────────────────────────────────────────────────────────

function CycleGraphPopup({
  onClose,
}: {
  onClose: () => void;
}) {
  const clockMHz = useStore((s) => s.clockMHz);
  const setClockMHz = useStore((s) => s.setClockMHz);
  const rawHistory = useStore((s) => s.cycleHistory);
  const arch = useStore((s) => s.arch);
  const [hiddenStages, setHiddenStages] = useState<Set<string>>(new Set());

  const history = rawHistory.length > MAX_DISPLAY
    ? rawHistory.slice(-MAX_DISPLAY)
    : rawHistory;
  const truncated = rawHistory.length > MAX_DISPLAY;
  const minCycle = history[0]?.cycle ?? 1;

  const exportCsv = useCallback(() => {
    if (history.length === 0) return;
    const tpc = 1000 / clockMHz;
    const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
    const lines = [
      ["cycle", "stage", "instruction", "time_offset_ns", "action"].join(","),
      ...history.map((e) =>
        [
          e.cycle,
          esc(e.stage),
          esc(getMnemonic(e.instructionBits, arch)),
          ((e.cycle - minCycle) * tpc).toFixed(1),
          esc(e.action || ""),
        ].join(",")
      ),
    ];
    const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `pipeline-timing-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [history, clockMHz, arch, minCycle]);

  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);

  const toggleStage = (s: string) =>
    setHiddenStages((prev) => {
      const next = new Set(prev);
      next.has(s) ? next.delete(s) : next.add(s);
      return next;
    });

  const presentStages = STAGE_ORDER.filter((s) => history.some((e) => e.stage === s));

  return (
    <div
      className="cycle-graph-overlay"
      role="presentation"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        className="cycle-graph-popup"
        role="dialog"
        aria-modal
        aria-label="Pipeline timing graph"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <header className="cycle-graph-header">
          <div className="cycle-graph-title-block">
            <h2 className="cycle-graph-title">Pipeline Timing</h2>
            <p className="cycle-graph-subtitle">
              Stage-by-stage execution · real simulated time · scroll-wheel zoom
            </p>
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
            {history.length > 0 && (
              <button type="button" className="btn btn-small" onClick={exportCsv}>
                Export CSV
              </button>
            )}
            <button
              type="button"
              className="cycle-graph-close"
              onClick={onClose}
              aria-label="Close (Esc)"
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
                  <rect x="3" y="3" width="18" height="3" rx="1" />
                  <rect x="3" y="9" width="14" height="3" rx="1" />
                  <rect x="3" y="15" width="10" height="3" rx="1" />
                  <rect x="3" y="21" width="16" height="3" rx="1" />
                </svg>
              </div>
              <p className="cycle-graph-empty-title">No cycle data yet</p>
              <p className="cycle-graph-empty-text">
                Assemble, then <strong>Step</strong> or <strong>Run</strong> to record pipeline execution.
              </p>
            </div>
          ) : (
            <>
              {/* Stats bar */}
              <StatsBar history={history} clockMHz={clockMHz} arch={arch} />

              {/* Stage filter */}
              <div className="gantt-filter-row">
                <span className="gantt-filter-label">Stage lanes:</span>
                {presentStages.map((s) => (
                  <button
                    key={s}
                    type="button"
                    className={`gantt-filter-btn${hiddenStages.has(s) ? " gantt-filter-btn--off" : ""}`}
                    style={
                      hiddenStages.has(s)
                        ? {}
                        : { borderColor: STAGE_COLORS[s], color: STAGE_COLORS[s] }
                    }
                    onClick={() => toggleStage(s)}
                  >
                    {s}
                  </button>
                ))}
                {truncated && (
                  <span className="gantt-truncated-note">
                    Showing last {MAX_DISPLAY} of {rawHistory.length} — export CSV for full data
                  </span>
                )}
              </div>

              {/* Pipeline Gantt */}
              <GanttChart
                history={history}
                clockMHz={clockMHz}
                arch={arch}
                hiddenStages={hiddenStages}
              />

              {/* Instruction summary */}
              <InstrSummaryTable history={history} clockMHz={clockMHz} arch={arch} />

              {/* Raw log (collapsible) */}
              <RawCyclesTable history={history} clockMHz={clockMHz} arch={arch} minCycle={minCycle} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Main ClockPanel (dock strip) ─────────────────────────────────────────────

export function ClockPanel() {
  const snapshot    = useStore((s) => s.snapshot);
  const speed       = useStore((s) => s.speed);
  const clockMHz    = useStore((s) => s.clockMHz);
  const cycleGraphOpen = useStore((s) => s.cycleGraphOpen);
  const setCycleHistoryGraphOpen = useStore((s) => s.setCycleHistoryGraphOpen);
  const historyLen  = useStore((s) => s.cycleHistory.length);

  const cycles   = snapshot?.total_cycles ?? 0;
  const runState = snapshot?.run_state ?? "IDLE";
  const timePerCycleNs = 1000 / clockMHz;

  return (
    <>
      <div className="clock-panel">
        <div className="clock-items-left">
          <div className="clock-item">
            <span className="clock-label">Cycles</span>
            <span className="clock-value">{cycles.toLocaleString()}</span>
          </div>
          <div className="clock-item">
            <span className="clock-label">State</span>
            <span className={`clock-value state-${runState.toLowerCase()}`}>{runState}</span>
          </div>
          <div className="clock-item">
            <span className="clock-label">Sim time</span>
            <span className="clock-value clock-value--dim">{fmtNs(cycles * timePerCycleNs)}</span>
          </div>
        </div>

        <button
          type="button"
          className="clock-graph-trigger"
          onClick={() => setCycleHistoryGraphOpen(!cycleGraphOpen)}
          title={cycleGraphOpen ? "Close pipeline timing graph (Esc)" : "Open pipeline timing graph"}
          aria-expanded={cycleGraphOpen}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75">
            <rect x="3" y="12" width="4" height="9" rx="1" />
            <rect x="10" y="7" width="4" height="14" rx="1" />
            <rect x="17" y="3" width="4" height="18" rx="1" />
          </svg>
          <span className="clock-graph-cycle">{historyLen} recorded</span>
          <span className="clock-graph-arrow" aria-hidden>{cycleGraphOpen ? "▼" : "▲"}</span>
        </button>

        <div className="clock-items-right">
          <div className="clock-item">
            <span className="clock-label">Clock</span>
            <span className="clock-value">{clockMHz} MHz</span>
          </div>
          <div className="clock-item">
            <span className="clock-label">Tick</span>
            <span className="clock-value">{fmtNs(timePerCycleNs)}/cycle</span>
          </div>
          <div className="clock-item">
            <span className="clock-label">Speed</span>
            <span className="clock-value clock-value--dim">{speed}ms</span>
          </div>
        </div>
      </div>

      {cycleGraphOpen && (
        <CycleGraphPopup onClose={() => setCycleHistoryGraphOpen(false)} />
      )}
    </>
  );
}
