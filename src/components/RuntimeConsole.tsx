import { useState, useRef, useEffect } from "react";
import { useStore } from "../store";

export function RuntimeConsole() {
  const snapshot = useStore((s) => s.snapshot);
  const panelVisibility = useStore((s) => s.panelVisibility);
  const stepForwardWithInput = useStore((s) => s.stepForwardWithInput);
  const [inputValue, setInputValue] = useState("");
  // Local clear offset: hide everything before this char index
  const [clearOffset, setClearOffset] = useState(0);
  const [copied, setCopied] = useState(false);
  const outputRef = useRef<HTMLPreElement>(null);

  const runError = snapshot?.run_error;
  const rawOutput = snapshot?.io_output ?? "";
  // Show output after the clear offset
  const ioOutput = rawOutput.substring(clearOffset);
  const ioInputRequested = snapshot?.io_input_requested;

  // Reset clear offset when simulator resets (output shrinks)
  useEffect(() => {
    if (rawOutput.length < clearOffset) setClearOffset(0);
  }, [rawOutput.length, clearOffset]);

  // Auto-scroll output to bottom on new content
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [ioOutput]);

  const handleInputSubmit = (val?: string) => {
    stepForwardWithInput(val ?? inputValue);
    setInputValue("");
  };

  const handleClear = () => {
    setClearOffset(rawOutput.length);
  };

  const handleCopy = async () => {
    if (!ioOutput) return;
    try {
      await navigator.clipboard.writeText(ioOutput);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // fallback — select all in the pre
      if (outputRef.current) {
        const sel = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(outputRef.current);
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
    }
  };

  const outputNeeded = ioOutput.length > 0;
  const inputNeeded = !!ioInputRequested;
  const showOutputSection = panelVisibility.output && (outputNeeded || rawOutput.length > 0);
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
                <div className="runtime-console-actions">
                  <button
                    type="button"
                    className="console-action-btn"
                    onClick={handleCopy}
                    disabled={!ioOutput}
                    title="Copy output to clipboard"
                  >
                    {copied ? "✓ Copied" : "Copy"}
                  </button>
                  <button
                    type="button"
                    className="console-action-btn console-action-btn--danger"
                    onClick={handleClear}
                    disabled={!outputNeeded}
                    title="Clear displayed output"
                  >
                    Clear
                  </button>
                </div>
              </div>
              <div className="runtime-console-body">
                {outputNeeded ? (
                  <pre ref={outputRef} className="runtime-console-io">{ioOutput}</pre>
                ) : (
                  <p className="runtime-console-cleared">Output cleared. New output will appear here.</p>
                )}
                <div className="runtime-console-hint">
                  RV32I: a7=11 print int, a7=12 print char · MIPS: v0=1 print int, v0=11 print char · LC-3: TRAP x20 OUT
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
                      <button type="button" onClick={() => handleInputSubmit("\n")} className="btn btn-small" title="Send newline">↵</button>
                      <button type="button" onClick={() => handleInputSubmit("0")} className="btn btn-small" title="Send '0'">0</button>
                    </>
                  )}
                  {ioInputRequested.kind === "int" && (
                    <button type="button" onClick={() => handleInputSubmit("0")} className="btn btn-small" title="Send 0">0</button>
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
                  Assembly succeeded but execution failed. Check the PC and the instruction at the error location.
                </div>
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}
