import { useStore } from "../store";
import type { TraceEvent } from "../types";

const EVENT_META: Record<TraceEvent, { label: string; color: string; bg: string }> = {
  FETCH:     { label: "FETCH",     color: "#60a5fa", bg: "rgba(59,130,246,0.12)"  },
  DECODE:    { label: "DECODE",    color: "#a78bfa", bg: "rgba(139,92,246,0.12)"  },
  ALU:       { label: "ALU",       color: "#34d399", bg: "rgba(16,185,129,0.12)"  },
  MEM:       { label: "MEM",       color: "#fbbf24", bg: "rgba(245,158,11,0.12)"  },
  REG_WRITE: { label: "REG_WRITE", color: "#22d3ee", bg: "rgba(6,182,212,0.12)"   },
  HALTED:    { label: "HALTED",    color: "#f87171", bg: "rgba(239,68,68,0.12)"   },
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
