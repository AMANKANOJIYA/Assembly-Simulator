import { useStore } from "../store";

export function TracePanel() {
  const traceEvents = useStore((s) => s.snapshot?.trace_events ?? []);

  return (
    <div className="panel trace-panel">
      <div className="panel-header">
        <h3>Trace / Events</h3>
      </div>
      <div className="trace-list">
        {traceEvents.length === 0 ? (
          <div className="trace-empty">No events yet</div>
        ) : (
          traceEvents.map((e, i) => (
            <div key={i} className="trace-item">
              {e}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
