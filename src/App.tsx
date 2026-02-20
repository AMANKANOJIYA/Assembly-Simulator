import { useEffect } from "react";
import { SplitPane, Pane } from "react-split-pane";
import "react-split-pane/styles.css";
import { EditorPanel } from "./components/Editor";
import { FileMenu } from "./components/FileMenu";
import { Controls } from "./components/Controls";
import { ClockPanel } from "./components/ClockPanel";
import { DiagramPanel } from "./components/DiagramPanel";
import { RegistersPanel } from "./components/RegistersPanel";
import { MemoryPanel } from "./components/MemoryPanel";
import { TracePanel } from "./components/TracePanel";
import { PanelWithMove } from "./components/PanelWithMove";
import { Toast } from "./components/Toast";
import { RuntimeConsole } from "./components/RuntimeConsole";
import { HelpPanel } from "./components/HelpPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { useStore } from "./store";
import { loadSession, saveSession } from "./store";
import { getDefaultSample } from "./samples";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./App.css";

function App() {
  const setSource = useStore((s) => s.setSource);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const assemble = useStore((s) => s.assemble);
  const loadSchemas = useStore((s) => s.loadSchemas);
  const refreshState = useStore((s) => s.refreshState);
  const panelVisibility = useStore((s) => s.panelVisibility);

  const saveFile = useStore((s) => s.saveFile);
  const loadFile = useStore((s) => s.loadFile);
  const newFile = useStore((s) => s.newFile);

  useEffect(() => {
    loadSession();
    setSource(getDefaultSample(useStore.getState().arch));
    loadSchemas();
    refreshState();
  }, [setSource, loadSchemas, refreshState]);

  useEffect(() => {
    const interval = setInterval(saveSession, 5000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        saveFile();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "o") {
        e.preventDefault();
        loadFile();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "n") {
        e.preventDefault();
        newFile();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [saveFile, loadFile, newFile]);

  const showTrace = panelVisibility.trace;
  const showRegisters = panelVisibility.registers;
  const showMemory = panelVisibility.memory;
  const panelOrder = useStore((s) => s.panelOrder);
  const customizeMode = useStore((s) => s.customizeMode);
  const movePanel = useStore((s) => s.movePanel);

  const panelConfigs = {
    registers: { visible: showRegisters, Comp: RegistersPanel, label: "Registers" },
    memory: { visible: showMemory, Comp: MemoryPanel, label: "Memory" },
    trace: { visible: showTrace, Comp: TracePanel, label: "Trace" },
  };

  return (
    <ErrorBoundary>
    <div className="app">
      <header className="app-header">
        <div className="app-header-left">
          <div className="app-logo" aria-hidden />
          <h1>Assembly Simulator</h1>
        </div>
        <FileMenu />
        <Controls />
        <button
          type="button"
          className="btn btn-icon app-settings-btn"
          onClick={() => setSettingsOpen(true)}
          title="Settings"
          aria-label="Settings"
        >
          ⚙
        </button>
      </header>

      <div className="main-layout">
        <SplitPane direction="horizontal" className="top-split">
          {/* Left: Architecture + Registers/Memory/Trace */}
          <Pane defaultSize="55%" minSize={250}>
            <SplitPane direction="vertical">
              <Pane defaultSize="40%" minSize={140}>
                <DiagramPanel />
              </Pane>
              <Pane>
                <SplitPane direction="horizontal">
                  {panelOrder[0] && (
                    <Pane defaultSize={panelConfigs[panelOrder[0]].visible ? "50%" : "0%"} minSize={panelConfigs[panelOrder[0]].visible ? 120 : 0}>
                      {panelConfigs[panelOrder[0]].visible ? (
                        <PanelWithMove id={panelOrder[0]} label={panelConfigs[panelOrder[0]].label} customizeMode={customizeMode} movePanel={movePanel}>
                          {panelOrder[0] === "registers" && <RegistersPanel />}
                          {panelOrder[0] === "memory" && <MemoryPanel />}
                          {panelOrder[0] === "trace" && <TracePanel />}
                        </PanelWithMove>
                      ) : <div className="panel-filler" />}
                    </Pane>
                  )}
                  <Pane>
                    <SplitPane direction="vertical">
                      {panelOrder[1] && (
                        <Pane defaultSize={panelConfigs[panelOrder[1]].visible ? "50%" : "0%"} minSize={panelConfigs[panelOrder[1]].visible ? 80 : 0}>
                          {panelConfigs[panelOrder[1]].visible ? (
                            <PanelWithMove id={panelOrder[1]} label={panelConfigs[panelOrder[1]].label} customizeMode={customizeMode} movePanel={movePanel}>
                              {panelOrder[1] === "registers" && <RegistersPanel />}
                              {panelOrder[1] === "memory" && <MemoryPanel />}
                              {panelOrder[1] === "trace" && <TracePanel />}
                            </PanelWithMove>
                          ) : <div className="panel-filler" />}
                        </Pane>
                      )}
                      {panelOrder[2] && (
                        <Pane defaultSize={panelConfigs[panelOrder[2]].visible ? "50%" : "0%"} minSize={panelConfigs[panelOrder[2]].visible ? 80 : 0}>
                          {panelConfigs[panelOrder[2]].visible ? (
                            <PanelWithMove id={panelOrder[2]} label={panelConfigs[panelOrder[2]].label} customizeMode={customizeMode} movePanel={movePanel}>
                              {panelOrder[2] === "registers" && <RegistersPanel />}
                              {panelOrder[2] === "memory" && <MemoryPanel />}
                              {panelOrder[2] === "trace" && <TracePanel />}
                            </PanelWithMove>
                          ) : <div className="panel-filler" />}
                        </Pane>
                      )}
                    </SplitPane>
                  </Pane>
                </SplitPane>
              </Pane>
            </SplitPane>
          </Pane>
          {/* Right: Code */}
          <Pane defaultSize="45%" minSize={220}>
            <EditorPanel onAssemble={() => assemble()} />
          </Pane>
        </SplitPane>
      </div>

      <RuntimeConsole />
      <footer className="app-footer">
        <ClockPanel />
      </footer>

      <Toast />
      <HelpPanel />
      <SettingsPanel />
    </div>
    </ErrorBoundary>
  );
}

export default App;
