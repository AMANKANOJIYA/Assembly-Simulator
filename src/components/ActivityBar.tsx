import { useStore } from "../store";
import { NavigatorFileActions } from "./NavigatorFileActions";

function IconDiagram() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="8.5" y="14" width="7" height="7" rx="1" />
      <path d="M6.5 10v2.5a2 2 0 0 0 2 2h3M17.5 10v1a2 2 0 0 1-2 2h-3" />
    </svg>
  );
}

function IconBottomPanel() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <rect x="2" y="3" width="20" height="14" rx="1" />
      <path d="M2 15h20" />
      <path d="M6 18h.01M10 18h4" />
    </svg>
  );
}

function IconSettings() {
  return (
    <svg className="activity-bar-svg activity-bar-svg--lg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.1a2 2 0 0 1-1-1.72v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

function IconShortcuts() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <rect x="2" y="5" width="20" height="14" rx="2" />
      <path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M8 13h.01M12 13h4M6 17h12" />
    </svg>
  );
}

/** VS Code–style: files in navigator, diagram, bottom panel, settings at bottom */
export function ActivityBar() {
  const diagramPanelOpen = useStore((s) => s.diagramPanelOpen);
  const setDiagramPanelOpen = useStore((s) => s.setDiagramPanelOpen);
  const bottomPanelOpen = useStore((s) => s.bottomPanelOpen);
  const setBottomPanelOpen = useStore((s) => s.setBottomPanelOpen);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const setShortcutsOpen = useStore((s) => s.setShortcutsOpen);

  return (
    <aside className="activity-bar" aria-label="Activity bar">
      <NavigatorFileActions />
      <div className="activity-bar-divider" role="separator" aria-hidden />
      <button
        type="button"
        className={`activity-bar-btn activity-bar-btn--icon${diagramPanelOpen ? " is-active" : ""}`}
        title={diagramPanelOpen ? "Hide architecture diagram" : "Show architecture diagram"}
        aria-pressed={diagramPanelOpen}
        onClick={() => setDiagramPanelOpen(!diagramPanelOpen)}
      >
        <span className="activity-bar-icon-wrap" aria-hidden>
          <IconDiagram />
        </span>
        <span className="activity-bar-label">Diagram</span>
      </button>
      <button
        type="button"
        className={`activity-bar-btn activity-bar-btn--icon${bottomPanelOpen ? " is-active" : ""}`}
        title={bottomPanelOpen ? "Hide bottom panel (I/O & clock)" : "Show bottom panel"}
        aria-pressed={bottomPanelOpen}
        onClick={() => setBottomPanelOpen(!bottomPanelOpen)}
      >
        <span className="activity-bar-icon-wrap" aria-hidden>
          <IconBottomPanel />
        </span>
        <span className="activity-bar-label">Panel</span>
      </button>
      <div className="activity-bar-spacer" />
      <button
        type="button"
        className="activity-bar-btn activity-bar-btn--icon"
        title="Keyboard shortcuts (?)"
        aria-label="Keyboard shortcuts"
        onClick={() => setShortcutsOpen(true)}
      >
        <span className="activity-bar-icon-wrap" aria-hidden>
          <IconShortcuts />
        </span>
        <span className="activity-bar-label">Shortcuts</span>
      </button>
      <button
        type="button"
        className="activity-bar-btn activity-bar-btn--icon activity-bar-btn-settings"
        data-tour="settings"
        title="Settings"
        aria-label="Settings"
        onClick={() => setSettingsOpen(true)}
      >
        <span className="activity-bar-icon-wrap activity-bar-icon-wrap--settings" aria-hidden>
          <IconSettings />
        </span>
        <span className="activity-bar-label">Settings</span>
      </button>
    </aside>
  );
}
