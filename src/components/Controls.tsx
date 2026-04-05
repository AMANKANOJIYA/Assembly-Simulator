import { useState, useRef, useEffect } from "react";
import { useStore } from "../store";
import {
  PlayIcon,
  PauseIcon,
  StopIcon,
  ResetIcon,
  StepForwardIcon,
  StepBackIcon,
} from "./Icons";

const ARCH_META: Record<string, { label: string; short: string; color: string }> = {
  RV32I:  { label: "RISC-V RV32I", short: "RV32I", color: "#3b82f6" },
  LC3:    { label: "LC-3",          short: "LC-3",  color: "#8b5cf6" },
  MIPS:   { label: "MIPS32",        short: "MIPS",  color: "#10b981" },
  "8085": { label: "Intel 8085",    short: "8085",  color: "#f59e0b" },
  "6502": { label: "MOS 6502",      short: "6502",  color: "#ef4444" },
  "8086": { label: "Intel 8086",    short: "8086",  color: "#f97316" },
};

function CtrlDivider() {
  return <span className="ctrl-divider" aria-hidden="true" />;
}

/** Custom ISA picker — colored pill that opens a styled dropdown */
function ArchPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (arch: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const meta = ARCH_META[value] ?? { label: value, short: value, color: "#6b7280" };

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open]);

  return (
    <div className="arch-picker" ref={ref} data-tour="arch-select">
      <button
        type="button"
        className="arch-pill"
        style={
          {
            "--arch-color": meta.color,
            "--arch-color-dim": `${meta.color}22`,
          } as React.CSSProperties
        }
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Architecture: ${meta.label}`}
        title="Change ISA"
      >
        <span className="arch-pill-dot" style={{ background: meta.color }} />
        <span className="arch-pill-short">{meta.short}</span>
        <svg className="arch-pill-chevron" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2.5">
          <path d="M6 9l6 6 6-6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <ul className="arch-dropdown" role="listbox" aria-label="Select architecture">
          {Object.entries(ARCH_META).map(([key, m]) => (
            <li
              key={key}
              role="option"
              aria-selected={key === value}
              className={`arch-option${key === value ? " arch-option--active" : ""}`}
              onClick={() => { onChange(key); setOpen(false); }}
            >
              <span className="arch-option-dot" style={{ background: m.color }} />
              <span className="arch-option-short">{m.short}</span>
              <span className="arch-option-label">{m.label}</span>
              {key === value && (
                <svg className="arch-option-check" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2.5">
                  <path d="M20 6L9 17l-5-5" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Map raw speed (ms/tick) → a 0-100 slider value, log-scaled so the slider feels linear */
function speedToSlider(ms: number): number {
  const clamped = Math.min(10_000_000, Math.max(1, ms));
  return Math.round(100 - (Math.log(clamped) / Math.log(10_000_000)) * 100);
}

function sliderToSpeed(val: number): number {
  return Math.round(Math.pow(10_000_000, (100 - val) / 100));
}

function speedLabel(ms: number): string {
  if (ms <= 1)       return "Max";
  if (ms < 1_000)    return `${ms} ms`;
  if (ms < 10_000)   return `${(ms / 1_000).toFixed(1)} s`;
  return "Slow";
}

export function Controls() {
  const arch        = useStore((s) => s.arch);
  const setArch     = useStore((s) => s.setArch);
  const loadSchemas = useStore((s) => s.loadSchemas);
  const snapshot    = useStore((s) => s.snapshot);
  const runIntervalId = useStore((s) => s.runIntervalId);
  const speed       = useStore((s) => s.speed);
  const setSpeed    = useStore((s) => s.setSpeed);
  const assemble    = useStore((s) => s.assemble);
  const run         = useStore((s) => s.run);
  const pause       = useStore((s) => s.pause);
  const stepForward = useStore((s) => s.stepForward);
  const stepBack    = useStore((s) => s.stepBack);
  const reset       = useStore((s) => s.reset);

  const handleArchChange = (newArch: string) => {
    setArch(newArch);
    loadSchemas(newArch);
  };

  const isRunning   = runIntervalId != null;
  const halted      = snapshot?.halted ?? false;
  const canStepBack = snapshot?.can_step_back ?? false;

  return (
    <div className="controls" role="toolbar" aria-label="Simulator controls">

      {/* ── ISA picker ───────────────────────────────────────────── */}
      <ArchPicker value={arch} onChange={handleArchChange} />

      <CtrlDivider />

      {/* ── Assemble ─────────────────────────────────────────────── */}
      <div className="ctrl-group">
        <button
          data-tour="assemble"
          onClick={() => assemble()}
          className="btn btn-primary btn-assemble"
          title="Assemble"
        >
          Assemble
        </button>
      </div>

      <CtrlDivider />

      {/* ── Execution controls ───────────────────────────────────── */}
      <div className="ctrl-group ctrl-group--exec" role="group" aria-label="Execution">
        <button
          data-tour="step-back"
          onClick={stepBack}
          disabled={!canStepBack || isRunning}
          className="btn btn-icon"
          title="Step Back"
          aria-label="Step back"
        >
          <StepBackIcon />
        </button>

        <button
          data-tour="run-pause"
          onClick={isRunning ? pause : run}
          disabled={halted}
          className={`btn btn-icon btn-run ${isRunning ? "btn-run--active" : ""}`}
          title={isRunning ? "Pause" : "Run"}
          aria-label={isRunning ? "Pause" : "Run"}
          aria-pressed={isRunning}
        >
          {isRunning ? <PauseIcon /> : <PlayIcon />}
        </button>

        <button
          data-tour="step-forward"
          onClick={stepForward}
          disabled={halted || !snapshot}
          className="btn btn-icon"
          title="Step Forward"
          aria-label="Step forward"
        >
          <StepForwardIcon />
        </button>

        <button
          data-tour="stop"
          onClick={pause}
          className="btn btn-icon"
          title="Stop"
          aria-label="Stop"
        >
          <StopIcon />
        </button>

        <button
          data-tour="reset"
          onClick={reset}
          className="btn btn-icon"
          title="Reset"
          aria-label="Reset"
        >
          <ResetIcon />
        </button>
      </div>

      <CtrlDivider />

      {/* ── Speed slider ─────────────────────────────────────────── */}
      <div className="ctrl-group ctrl-group--speed" title="Execution speed">
        <span className="speed-label" aria-hidden="true">Speed</span>
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={speedToSlider(speed)}
          onChange={(e) => setSpeed(sliderToSpeed(Number(e.target.value)))}
          className="speed-slider"
          aria-label="Execution speed"
          aria-valuetext={speedLabel(speed)}
        />
        <span className="speed-value">{speedLabel(speed)}</span>
      </div>
    </div>
  );
}
