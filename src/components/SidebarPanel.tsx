import { useEffect } from "react";
import { useStore } from "../store";
import type { PanelId } from "../store";
import { RegistersPanel } from "./RegistersPanel";
import { MemoryPanel } from "./MemoryPanel";
import { TracePanel } from "./TracePanel";

const LABELS: Record<PanelId, string> = {
  registers: "Registers",
  memory: "Memory",
  trace: "Trace",
};

function TabIcon({ id }: { id: PanelId }) {
  const cls = "sidebar-unified-tab-ico";
  if (id === "registers") {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
        <rect x="4" y="4" width="6" height="6" rx="1" />
        <rect x="14" y="4" width="6" height="6" rx="1" />
        <rect x="4" y="14" width="6" height="6" rx="1" />
        <rect x="14" y="14" width="6" height="6" rx="1" />
      </svg>
    );
  }
  if (id === "memory") {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
        <rect x="3" y="3" width="18" height="18" rx="1" />
        <path d="M3 9h18M9 3v18" />
      </svg>
    );
  }
  return (
    <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <path d="M4 6h16M4 12h10M4 18h14" />
      <circle cx="18" cy="12" r="2" fill="currentColor" stroke="none" />
    </svg>
  );
}

/** Single tab navigator for the three data panels (replaces multi-split clutter) */
export function SidebarPanel() {
  const panelVisibility = useStore((s) => s.panelVisibility);
  const sidebarView = useStore((s) => s.sidebarView);
  const setSidebarView = useStore((s) => s.setSidebarView);

  const available = (["registers", "memory", "trace"] as const).filter((id) => panelVisibility[id]);

  useEffect(() => {
    if (!panelVisibility[sidebarView]) {
      const first = (["registers", "memory", "trace"] as const).find((id) => panelVisibility[id]);
      if (first) setSidebarView(first);
    }
  }, [panelVisibility, sidebarView, setSidebarView]);

  return (
    <div className="sidebar-unified">
      <div className="sidebar-unified-tabs" role="tablist" aria-label="Simulator panels">
        {available.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={sidebarView === id}
            className={`sidebar-unified-tab${sidebarView === id ? " is-active" : ""}`}
            onClick={() => setSidebarView(id)}
          >
            <TabIcon id={id} />
            <span className="sidebar-unified-tab-text">{LABELS[id]}</span>
          </button>
        ))}
      </div>
      <div className="sidebar-unified-body">
        {available.length === 0 ? (
          <div className="sidebar-unified-empty">Enable panels in Settings</div>
        ) : (
          <>
            {sidebarView === "registers" && panelVisibility.registers && <RegistersPanel />}
            {sidebarView === "memory" && panelVisibility.memory && <MemoryPanel />}
            {sidebarView === "trace" && panelVisibility.trace && <TracePanel />}
          </>
        )}
      </div>
    </div>
  );
}
