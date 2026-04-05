import { useEffect } from "react";
import { SplitPane, Pane } from "react-split-pane";
import "react-split-pane/styles.css";
import { EditorPanel } from "./components/Editor";
import { ThemeToggleButton } from "./components/ThemeToggleButton";
import { Controls } from "./components/Controls";
import { ClockPanel } from "./components/ClockPanel";
import { DiagramPanel } from "./components/DiagramPanel";
import { Toast } from "./components/Toast";
import { RuntimeConsole } from "./components/RuntimeConsole";
import { HelpPanel } from "./components/HelpPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { OnboardingTour, hasCompletedOnboarding } from "./components/OnboardingTour";
import { useStore } from "./store";
import { loadSession, saveSession } from "./store";
import { getDefaultSample } from "./samples";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./App.css";
import "./layout-shell.css";
import { ActivityBar } from "./components/ActivityBar";
import { StatusStrip } from "./components/StatusStrip";
import { SidebarPanel } from "./components/SidebarPanel";
import { AppearanceSync } from "./components/AppearanceSync";

function LeftColumnStack() {
  const diagramPanelOpen = useStore((s) => s.diagramPanelOpen);
  const panel = (
    <div className="left-column-fill">
      <SidebarPanel />
    </div>
  );

  if (!diagramPanelOpen) {
    return panel;
  }

  return (
    <SplitPane direction="vertical" className="diagram-stack-split">
      <Pane defaultSize="40%" minSize="120px">
        <DiagramPanel />
      </Pane>
      <Pane defaultSize="60%" minSize="160px">
        {panel}
      </Pane>
    </SplitPane>
  );
}

function MainWorkspace() {
  const bottomPanelOpen = useStore((s) => s.bottomPanelOpen);
  const setBottomPanelOpen = useStore((s) => s.setBottomPanelOpen);
  const snapshot = useStore((s) => s.snapshot);

  useEffect(() => {
    if (snapshot?.io_input_requested) {
      setBottomPanelOpen(true);
    }
  }, [snapshot?.io_input_requested, setBottomPanelOpen]);

  const center = (
    <SplitPane direction="horizontal" className="top-split">
      <Pane defaultSize="55%" minSize="200px">
        <LeftColumnStack />
      </Pane>
      <Pane defaultSize="45%" minSize="220px">
        <EditorPanel />
      </Pane>
    </SplitPane>
  );

  const bottomDock = (
    <div className="bottom-dock">
      <RuntimeConsole />
      <footer className="app-footer">
        <ClockPanel />
      </footer>
    </div>
  );

  return (
    <div className="main-workspace-flex">
      <div className="main-workspace-center">{center}</div>
      {!bottomPanelOpen && (
        <button
          type="button"
          className="bottom-panel-reveal"
          onClick={() => setBottomPanelOpen(true)}
          aria-label="Show bottom panel: console, I/O, and clock"
        >
          <span className="bottom-panel-reveal-icon" aria-hidden>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 15l-6-6-6 6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </span>
          <span className="bottom-panel-reveal-text">Console · I/O · Clock</span>
        </button>
      )}
      <div
        className={`main-workspace-bottom${bottomPanelOpen ? " main-workspace-bottom--open" : ""}`}
        aria-hidden={!bottomPanelOpen}
      >
        {bottomDock}
      </div>
    </div>
  );
}

function App() {
  const setSource = useStore((s) => s.setSource);
  const setOnboardingOpen = useStore((s) => s.setOnboardingOpen);
  const loadSchemas = useStore((s) => s.loadSchemas);
  const refreshState = useStore((s) => s.refreshState);

  const themeMode = useStore((s) => s.themeMode);
  useEffect(() => {
    document.documentElement.dataset.theme = themeMode;
  }, [themeMode]);

  useEffect(() => {
    loadSession();
    setSource(getDefaultSample(useStore.getState().arch));
    loadSchemas();
    refreshState();
    if (!hasCompletedOnboarding()) {
      setOnboardingOpen(true);
    }
  }, [setSource, loadSchemas, refreshState, setOnboardingOpen]);

  // Session auto-save: debounce on page visibility to avoid unnecessary writes
  useEffect(() => {
    const interval = setInterval(() => {
      if (!document.hidden) saveSession();
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  // Stable keyboard handler — reads store actions via getState() so the effect
  // never needs to re-run when those functions change reference.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === "s") {
        e.preventDefault();
        useStore.getState().saveFile();
      } else if (e.key === "o") {
        e.preventDefault();
        useStore.getState().loadFile();
      } else if (e.key === "n") {
        e.preventDefault();
        useStore.getState().newFile();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []); // empty deps — always reads latest via getState()

  return (
    <ErrorBoundary>
      <div className="app">
        <AppearanceSync />
        <header className="app-header">
          <div className="app-header-left">
            <img className="app-logo" src="/logo.png" alt="" decoding="async" />
            <h1>Assembly Simulator</h1>
          </div>
          <Controls />
          <div className="app-header-right">
            <ThemeToggleButton />
          </div>
        </header>

        <StatusStrip />

        <div className="app-body">
          <ActivityBar />
          <div className="app-workspace">
            <MainWorkspace />
          </div>
        </div>

        <Toast />
        <HelpPanel />
        <SettingsPanel />
        <OnboardingTour />
      </div>
    </ErrorBoundary>
  );
}

export default App;
