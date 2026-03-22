import { useState } from "react";
import { useStore } from "../store";

export function RuntimeConsole() {
  const snapshot = useStore((s) => s.snapshot);
  const panelVisibility = useStore((s) => s.panelVisibility);
  const stepForwardWithInput = useStore((s) => s.stepForwardWithInput);
  const [inputValue, setInputValue] = useState("");
  const runError = snapshot?.run_error;
  const ioOutput = snapshot?.io_output ?? "";
  const ioInputRequested = snapshot?.io_input_requested;

  const handleInputSubmit = (val?: string) => {
    stepForwardWithInput(val ?? inputValue);
    setInputValue("");
  };

  const outputNeeded = (ioOutput?.length ?? 0) > 0;
  const inputNeeded = !!ioInputRequested;
  const showOutputSection = panelVisibility.output && outputNeeded;
  const showInputSection = panelVisibility.input && inputNeeded;
  const showConsole = showOutputSection || showInputSection || !!runError;

  return (
    <div
      className={`runtime-console${!showConsole ? " runtime-console--idle" : ""}`}
      data-tour="runtime-console"
    >
      {!showConsole ? (
        <div className="runtime-console-placeholder">
          <p className="runtime-console-placeholder-title">Program output &amp; I/O</p>
          <p className="runtime-console-placeholder-text">
            When you run a program, output and trap/syscall activity appear here. If the panel looks empty, enable{" "}
            <strong>Program Output</strong> and <strong>Trap / Input</strong> under Settings → Panels.
          </p>
        </div>
      ) : (
        <>
          {showOutputSection && (
            <>
              <div className="runtime-console-header runtime-console-output">
                <span>Program Output</span>
              </div>
              <div className="runtime-console-body">
                <pre className="runtime-console-io">{ioOutput}</pre>
                <div className="runtime-console-hint">
                  RISC-V: a7=11 print int, a7=12 print char. MIPS: v0=1 print int, v0=11 print char. LC-3: TRAP x20 OUT.
                </div>
              </div>
            </>
          )}

          {showInputSection && ioInputRequested && (
            <>
              <div className="runtime-console-header runtime-console-input">
                <span>Trap / Interrupt Input</span>
                <span className="runtime-console-badge">Waiting for input</span>
              </div>
              <div className="runtime-console-body">
                <p className="runtime-console-prompt">{ioInputRequested.prompt}</p>
                <div className="runtime-console-input-row">
                  <input
                    type={ioInputRequested.kind === "int" ? "number" : "text"}
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleInputSubmit()}
                    maxLength={ioInputRequested.kind === "char" ? 1 : ioInputRequested.max_length ?? undefined}
                    placeholder={ioInputRequested.kind === "char" ? "char" : ioInputRequested.kind === "int" ? "integer" : "string"}
                    className="runtime-console-input-field"
                    autoFocus
                  />
                  <button type="button" onClick={() => handleInputSubmit()} className="btn btn-primary btn-send">
                    Send to Program
                  </button>
                  {ioInputRequested.kind === "char" && (
                    <>
                      <button type="button" onClick={() => handleInputSubmit("\n")} className="btn btn-small" title="Send newline">
                        ↵
                      </button>
                      <button type="button" onClick={() => handleInputSubmit("0")} className="btn btn-small" title="Send '0'">
                        0
                      </button>
                    </>
                  )}
                  {ioInputRequested.kind === "int" && (
                    <button type="button" onClick={() => handleInputSubmit("0")} className="btn btn-small" title="Send 0">
                      0
                    </button>
                  )}
                </div>
              </div>
            </>
          )}

          {runError && (
            <>
              <div className="runtime-console-header runtime-console-error">
                <span className="runtime-console-icon">◆</span>
                <span>Runtime Error</span>
              </div>
              <div className="runtime-console-body">
                <pre className="runtime-console-message">{runError}</pre>
                <div className="runtime-console-hint">
                  Assembly succeeded but execution failed. Check the PC (program counter) and instruction at the error location.
                </div>
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}
