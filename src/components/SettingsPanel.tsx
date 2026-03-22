import { useState, useMemo } from "react";
import { useStore } from "../store";
import {
  SAMPLE_FULL,
  SAMPLE_PRINT,
  SAMPLE_REGISTERS,
  SAMPLE_MEMORY,
  SAMPLE_BRANCH,
} from "../samples";
import type { ThemeMode } from "../store";
import type { UiFontId, MonoFontId } from "../appearance";
import {
  UI_FONT_LABELS,
  MONO_FONT_LABELS,
  EDITOR_FONT_SIZE,
} from "../appearance";

type SettingsSectionId = "commonly-used" | "appearance" | "simulator" | "panels" | "help";

const SECTIONS: { id: SettingsSectionId; label: string; description: string }[] = [
  { id: "commonly-used", label: "Commonly Used", description: "Speed and safety limits" },
  { id: "appearance", label: "Appearance", description: "Theme, fonts, density, motion" },
  { id: "simulator", label: "Simulator", description: "Run behavior" },
  { id: "panels", label: "Panels", description: "Visibility and layout" },
  { id: "help", label: "Help", description: "Documentation and tours" },
];

export function SettingsPanel() {
  const settingsOpen = useStore((s) => s.settingsOpen);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const setHelpOpen = useStore((s) => s.setHelpOpen);
  const setOnboardingOpen = useStore((s) => s.setOnboardingOpen);
  const speed = useStore((s) => s.speed);
  const setSpeed = useStore((s) => s.setSpeed);
  const panelVisibility = useStore((s) => s.panelVisibility);
  const setPanelVisibility = useStore((s) => s.setPanelVisibility);
  const customizeMode = useStore((s) => s.customizeMode);
  const setCustomizeMode = useStore((s) => s.setCustomizeMode);
  const maxCycleLimit = useStore((s) => s.maxCycleLimit);
  const setMaxCycleLimit = useStore((s) => s.setMaxCycleLimit);
  const setSource = useStore((s) => s.setSource);
  const themeMode = useStore((s) => s.themeMode);
  const setThemeMode = useStore((s) => s.setThemeMode);
  const uiFontFamily = useStore((s) => s.uiFontFamily);
  const setUiFontFamily = useStore((s) => s.setUiFontFamily);
  const monoFontFamily = useStore((s) => s.monoFontFamily);
  const setMonoFontFamily = useStore((s) => s.setMonoFontFamily);
  const editorFontSize = useStore((s) => s.editorFontSize);
  const setEditorFontSize = useStore((s) => s.setEditorFontSize);
  const uiDensity = useStore((s) => s.uiDensity);
  const setUiDensity = useStore((s) => s.setUiDensity);
  const reducedMotion = useStore((s) => s.reducedMotion);
  const setReducedMotion = useStore((s) => s.setReducedMotion);

  const [activeSection, setActiveSection] = useState<SettingsSectionId>("commonly-used");
  const [search, setSearch] = useState("");

  const filteredSections = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return SECTIONS;
    const f = SECTIONS.filter(
      (s) => s.label.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)
    );
    return f.length ? f : SECTIONS;
  }, [search]);

  if (!settingsOpen) return null;

  const openHelp = () => {
    setSettingsOpen(false);
    setHelpOpen(true);
  };

  return (
    <div
      className="vscode-settings-overlay"
      onClick={() => setSettingsOpen(false)}
      role="presentation"
    >
      <div
        className="vscode-settings"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Settings"
        aria-modal="true"
      >
        <header className="vscode-settings-topbar">
          <div className="vscode-settings-title-block">
            <h2 className="vscode-settings-title">Settings</h2>
            <p className="vscode-settings-subtitle">User — Assembly Simulator</p>
          </div>
          <div className="vscode-settings-search-wrap">
            <span className="vscode-settings-search-icon" aria-hidden>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.35-4.35" />
              </svg>
            </span>
            <input
              type="search"
              className="vscode-settings-search"
              placeholder="Search settings"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              aria-label="Search settings"
            />
          </div>
          <button
            type="button"
            className="vscode-settings-close"
            onClick={() => setSettingsOpen(false)}
            aria-label="Close settings"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        </header>

        <div className="vscode-settings-body">
          <nav className="vscode-settings-nav" aria-label="Settings categories">
            {filteredSections.map((s) => (
              <button
                key={s.id}
                type="button"
                className={`vscode-settings-nav-item${activeSection === s.id ? " is-active" : ""}`}
                onClick={() => setActiveSection(s.id)}
              >
                <span className="vscode-settings-nav-label">{s.label}</span>
                <span className="vscode-settings-nav-desc">{s.description}</span>
              </button>
            ))}
          </nav>

          <div className="vscode-settings-main">
            {activeSection === "commonly-used" && (
              <div className="vscode-settings-pane">
                <h3 className="vscode-settings-pane-title">Commonly Used</h3>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Run speed</span>
                    <span className="vscode-setting-desc">Delay between steps when running (milliseconds).</span>
                  </div>
                  <div className="vscode-setting-control">
                    <span className="vscode-setting-value">{speed} ms</span>
                    <input
                      type="range"
                      min={10}
                      max={500}
                      step={10}
                      value={speed}
                      onChange={(e) => setSpeed(Number(e.target.value))}
                      className="vscode-settings-slider"
                    />
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Max cycle limit</span>
                    <span className="vscode-setting-desc">Stop execution after a number of cycles (safety).</span>
                  </div>
                  <div className="vscode-setting-control vscode-setting-control--stack">
                    <label className="vscode-checkbox">
                      <input
                        type="checkbox"
                        checked={maxCycleLimit != null}
                        onChange={(e) => setMaxCycleLimit(e.target.checked ? 10000 : null)}
                      />
                      <span>Enable limit</span>
                    </label>
                    {maxCycleLimit != null && (
                      <label className="vscode-setting-inline-num">
                        <span>Limit</span>
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
                  </div>
                </div>
              </div>
            )}

            {activeSection === "appearance" && (
              <div className="vscode-settings-pane">
                <h3 className="vscode-settings-pane-title">Appearance</h3>
                <p className="vscode-settings-intro">
                  Adjust how text and spacing look across the app. Changes apply immediately and are saved for next time.
                </p>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Color theme</span>
                    <span className="vscode-setting-desc">Dark or light interface (also in the header).</span>
                  </div>
                  <div className="vscode-setting-control">
                    <select
                      className="vscode-select"
                      value={themeMode}
                      onChange={(e) => setThemeMode(e.target.value as ThemeMode)}
                      aria-label="Color theme"
                    >
                      <option value="dark">Dark</option>
                      <option value="light">Light</option>
                    </select>
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Interface font</span>
                    <span className="vscode-setting-desc">Used for menus, panels, and labels.</span>
                  </div>
                  <div className="vscode-setting-control">
                    <select
                      className="vscode-select"
                      value={uiFontFamily}
                      onChange={(e) => setUiFontFamily(e.target.value as UiFontId)}
                      aria-label="Interface font"
                    >
                      {(Object.keys(UI_FONT_LABELS) as UiFontId[]).map((id) => (
                        <option key={id} value={id}>
                          {UI_FONT_LABELS[id]}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Code font</span>
                    <span className="vscode-setting-desc">Monospace font for the editor and tables.</span>
                  </div>
                  <div className="vscode-setting-control">
                    <select
                      className="vscode-select"
                      value={monoFontFamily}
                      onChange={(e) => setMonoFontFamily(e.target.value as MonoFontId)}
                      aria-label="Code font"
                    >
                      {(Object.keys(MONO_FONT_LABELS) as MonoFontId[]).map((id) => (
                        <option key={id} value={id}>
                          {MONO_FONT_LABELS[id]}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Editor font size</span>
                    <span className="vscode-setting-desc">Size in pixels for the assembly editor.</span>
                  </div>
                  <div className="vscode-setting-control">
                    <span className="vscode-setting-value">{editorFontSize}px</span>
                    <input
                      type="range"
                      min={EDITOR_FONT_SIZE.min}
                      max={EDITOR_FONT_SIZE.max}
                      step={1}
                      value={editorFontSize}
                      onChange={(e) => setEditorFontSize(Number(e.target.value))}
                      className="vscode-settings-slider"
                      aria-valuemin={EDITOR_FONT_SIZE.min}
                      aria-valuemax={EDITOR_FONT_SIZE.max}
                      aria-label="Editor font size"
                    />
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">UI density</span>
                    <span className="vscode-setting-desc">Comfortable gives more padding; compact fits more on screen.</span>
                  </div>
                  <div className="vscode-setting-control">
                    <select
                      className="vscode-select"
                      value={uiDensity}
                      onChange={(e) => setUiDensity(e.target.value as "comfortable" | "compact")}
                      aria-label="UI density"
                    >
                      <option value="comfortable">Comfortable</option>
                      <option value="compact">Compact</option>
                    </select>
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Reduce motion</span>
                    <span className="vscode-setting-desc">Minimize animations and transitions (easier on eyes or vestibular sensitivity).</span>
                  </div>
                  <div className="vscode-setting-control">
                    <label className="vscode-checkbox">
                      <input
                        type="checkbox"
                        checked={reducedMotion}
                        onChange={(e) => setReducedMotion(e.target.checked)}
                      />
                      <span>Reduce UI animations</span>
                    </label>
                  </div>
                </div>
              </div>
            )}

            {activeSection === "simulator" && (
              <div className="vscode-settings-pane">
                <h3 className="vscode-settings-pane-title">Simulator</h3>
                <p className="vscode-settings-intro">
                  Architecture, assemble, and clock frequency are controlled from the top toolbar. Use{" "}
                  <strong>Commonly Used</strong> for run speed and cycle limits.
                </p>
              </div>
            )}

            {activeSection === "panels" && (
              <div className="vscode-settings-pane">
                <h3 className="vscode-settings-pane-title">Panels</h3>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Visible panels</span>
                    <span className="vscode-setting-desc">Show or hide simulator UI sections.</span>
                  </div>
                  <div className="vscode-setting-control vscode-setting-control--stack">
                    {(
                      [
                        ["registers", "Registers"],
                        ["memory", "Memory"],
                        ["trace", "Trace"],
                        ["output", "Program Output"],
                        ["input", "Trap / Input"],
                      ] as const
                    ).map(([key, label]) => (
                      <label key={key} className="vscode-checkbox">
                        <input
                          type="checkbox"
                          checked={panelVisibility[key]}
                          onChange={(e) => setPanelVisibility(key, e.target.checked)}
                        />
                        <span>{label}</span>
                      </label>
                    ))}
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Customize layout</span>
                    <span className="vscode-setting-desc">Allow dragging to reorder panels (when supported).</span>
                  </div>
                  <div className="vscode-setting-control">
                    <label className="vscode-checkbox">
                      <input
                        type="checkbox"
                        checked={customizeMode}
                        onChange={(e) => setCustomizeMode(e.target.checked)}
                      />
                      <span>Customize mode</span>
                    </label>
                  </div>
                </div>
              </div>
            )}

            {activeSection === "help" && (
              <div className="vscode-settings-pane">
                <h3 className="vscode-settings-pane-title">Help</h3>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Documentation</span>
                    <span className="vscode-setting-desc">Open the in-app help panel.</span>
                  </div>
                  <div className="vscode-setting-control">
                    <button type="button" className="btn btn-small" onClick={openHelp}>
                      Open Help
                    </button>
                  </div>
                </div>
                <div className="vscode-setting-row">
                  <div className="vscode-setting-label">
                    <span className="vscode-setting-name">Welcome tour</span>
                    <span className="vscode-setting-desc">Step through the main UI features.</span>
                  </div>
                  <div className="vscode-setting-control">
                    <button
                      type="button"
                      className="btn btn-small"
                      onClick={() => {
                        setSettingsOpen(false);
                        setOnboardingOpen(true);
                      }}
                    >
                      Start Tutorial
                    </button>
                  </div>
                </div>
                <div className="vscode-settings-divider" />
                <h4 className="vscode-settings-subheading">Quick samples (current buffer)</h4>
                <p className="vscode-settings-intro">Loads into the active editor tab. Use the navigator for more samples.</p>
                <div className="vscode-settings-sample-grid">
                  {(
                    [
                      ["Full demo", SAMPLE_FULL],
                      ["Print (ecall)", SAMPLE_PRINT],
                      ["Registers", SAMPLE_REGISTERS],
                      ["Memory", SAMPLE_MEMORY],
                      ["Branch", SAMPLE_BRANCH],
                    ] as const
                  ).map(([label, code]) => (
                    <button
                      key={label}
                      type="button"
                      className="btn btn-small"
                      onClick={() => {
                        setSource(code);
                        setSettingsOpen(false);
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
