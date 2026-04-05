import { useStore } from "../store";
import type { TraceEvent } from "../types";

// Colors match the Pipeline Timing Gantt stage palette exactly
const EVENT_META: Record<TraceEvent, { label: string; color: string; bg: string }> = {
  FETCH:     { label: "FETCH",     color: "#6366f1", bg: "rgba(99,102,241,0.15)"  },
  DECODE:    { label: "DECODE",    color: "#22c55e", bg: "rgba(34,197,94,0.15)"   },
  ALU:       { label: "ALU",       color: "#f59e0b", bg: "rgba(245,158,11,0.15)"  },
  MEM:       { label: "MEM",       color: "#ec4899", bg: "rgba(236,72,153,0.15)"  },
  REG_WRITE: { label: "REG_WRITE", color: "#3b82f6", bg: "rgba(59,130,246,0.15)"  },
  HALTED:    { label: "HALTED",    color: "#ef4444", bg: "rgba(239,68,68,0.15)"   },
};

function EventBadge({ event }: { event: TraceEvent }) {
  const meta = EVENT_META[event] ?? { label: event, color: "#9ca3af", bg: "rgba(156,163,175,0.12)" };
  return (
    <span
      className="trace-badge"
      style={{ color: meta.color, background: meta.bg, borderColor: meta.color }}
    >
      {meta.label}
    </span>
  );
}

export function TracePanel() {
  const traceEvents  = useStore((s) => s.snapshot?.trace_events ?? []);
  const totalCycles  = useStore((s) => s.snapshot?.total_cycles ?? 0);

  // Show newest first; each index within current cycle batch is a stage
  const reversed = [...traceEvents].reverse();

  return (
    <div className="panel trace-panel" data-tour="trace">
      <div className="panel-header">
        <h3 className="panel-title">Trace</h3>
        <span className="panel-badge">{traceEvents.length} events</span>
      </div>

      <div className="trace-list">
        {reversed.length === 0 ? (
          <div className="trace-empty">
            <span className="trace-empty-icon">⌀</span>
            Run or step to see trace events
          </div>
        ) : (
          reversed.map((e, i) => {
            const cycleNum = totalCycles - i;
            return (
              <div key={i} className="trace-row">
                <span className="trace-cycle">#{cycleNum > 0 ? cycleNum : "—"}</span>
                <EventBadge event={e} />
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
