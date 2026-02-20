import { useStore } from "../store";
import { SAMPLE_FULL, SAMPLE_PRINT, SAMPLE_REGISTERS, SAMPLE_MEMORY, SAMPLE_BRANCH } from "../samples";

export function SettingsPanel() {
  const settingsOpen = useStore((s) => s.settingsOpen);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const setHelpOpen = useStore((s) => s.setHelpOpen);
  const speed = useStore((s) => s.speed);
  const setSpeed = useStore((s) => s.setSpeed);
  const panelVisibility = useStore((s) => s.panelVisibility);
  const setPanelVisibility = useStore((s) => s.setPanelVisibility);
  const customizeMode = useStore((s) => s.customizeMode);
  const setCustomizeMode = useStore((s) => s.setCustomizeMode);
  const maxCycleLimit = useStore((s) => s.maxCycleLimit);
  const setMaxCycleLimit = useStore((s) => s.setMaxCycleLimit);
  const setSource = useStore((s) => s.setSource);

  if (!settingsOpen) return null;

  const openHelp = () => {
    setSettingsOpen(false);
    setHelpOpen(true);
  };

  return (
    <div
      className="settings-overlay"
      onClick={() => setSettingsOpen(false)}
      role="presentation"
    >
      <div
        className="settings-popup"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Settings"
      >
        <div className="settings-header">
          <h3>Settings</h3>
          <button
            type="button"
            className="btn btn-small btn-ghost"
            onClick={() => setSettingsOpen(false)}
            aria-label="Close"
          >
            ×
          </button>
        </div>
        <div className="settings-body">
          <section className="settings-section">
            <h4>Samples</h4>
            <div className="settings-samples">
              <button type="button" className="btn btn-small" onClick={() => { setSource(SAMPLE_FULL); setSettingsOpen(false); }}>
                Full demo
              </button>
              <button type="button" className="btn btn-small" onClick={() => { setSource(SAMPLE_PRINT); setSettingsOpen(false); }}>
                Print (ecall)
              </button>
              <button type="button" className="btn btn-small" onClick={() => { setSource(SAMPLE_REGISTERS); setSettingsOpen(false); }}>
                Registers
              </button>
              <button type="button" className="btn btn-small" onClick={() => { setSource(SAMPLE_MEMORY); setSettingsOpen(false); }}>
                Memory
              </button>
              <button type="button" className="btn btn-small" onClick={() => { setSource(SAMPLE_BRANCH); setSettingsOpen(false); }}>
                Branch
              </button>
            </div>
          </section>
          <section className="settings-section">
            <h4>Help</h4>
            <button type="button" className="btn btn-small" onClick={openHelp}>
              Open Help
            </button>
          </section>
          <section className="settings-section">
            <h4>Run Speed</h4>
            <label className="settings-speed-wrap">
              <span>{speed} ms</span>
              <input
                type="range"
                min={10}
                max={500}
                step={10}
                value={speed}
                onChange={(e) => setSpeed(Number(e.target.value))}
                className="settings-speed-slider"
              />
            </label>
          </section>
          <section className="settings-section">
            <h4>Panels</h4>
            <div className="settings-panels">
              {(
                [
                  ["registers", "Registers"],
                  ["memory", "Memory"],
                  ["trace", "Trace"],
                  ["output", "Program Output"],
                  ["input", "Trap / Input"],
                ] as const
              ).map(([key, label]) => (
                <label key={key} className="settings-panel-toggle">
                  <input
                    type="checkbox"
                    checked={panelVisibility[key]}
                    onChange={(e) => setPanelVisibility(key, e.target.checked)}
                  />
                  {label}
                </label>
              ))}
            </div>
          </section>
          <section className="settings-section">
            <h4>Max cycle limit</h4>
            <label className="settings-cycle-limit">
              <input
                type="checkbox"
                checked={maxCycleLimit != null}
                onChange={(e) => setMaxCycleLimit(e.target.checked ? 10000 : null)}
              />
              Enable (stops run at limit)
            </label>
            {maxCycleLimit != null && (
              <label className="settings-cycle-value">
                <span>Limit:</span>
                <input
                  type="number"
                  min={100}
                  max={10000000}
                  step={1000}
                  value={maxCycleLimit}
                  onChange={(e) => setMaxCycleLimit(Math.max(100, parseInt(e.target.value, 10) || 100))}
                />
              </label>
            )}
          </section>
          <section className="settings-section">
            <h4>Layout</h4>
            <label className="settings-move-toggle">
              <input
                type="checkbox"
                checked={customizeMode}
                onChange={(e) => setCustomizeMode(e.target.checked)}
              />
              Customize mode (drag to move panels)
            </label>
          </section>
        </div>
      </div>
    </div>
  );
}
