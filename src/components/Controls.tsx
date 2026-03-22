import { useStore } from "../store";
import {
  PlayIcon,
  PauseIcon,
  StopIcon,
  ResetIcon,
  StepForwardIcon,
  StepBackIcon,
} from "./Icons";

export function Controls() {
  const arch = useStore((s) => s.arch);
  const setArch = useStore((s) => s.setArch);
  const snapshot = useStore((s) => s.snapshot);
  const runIntervalId = useStore((s) => s.runIntervalId);
  const assemble = useStore((s) => s.assemble);
  const run = useStore((s) => s.run);
  const pause = useStore((s) => s.pause);
  const stepForward = useStore((s) => s.stepForward);
  const stepBack = useStore((s) => s.stepBack);
  const reset = useStore((s) => s.reset);
  const loadSchemas = useStore((s) => s.loadSchemas);

  const handleArchChange = (newArch: string) => {
    setArch(newArch);
    loadSchemas(newArch);
  };

  const isRunning = runIntervalId != null;
  const halted = snapshot?.halted ?? false;
  const canStepBack = snapshot?.can_step_back ?? false;

  return (
    <div className="controls">
      <div className="control-row">
        <select
          data-tour="arch-select"
          value={arch}
          onChange={(e) => handleArchChange(e.target.value)}
          className="arch-select"
        >
          <option value="RV32I">RISC-V RV32I</option>
          <option value="LC3">LC-3</option>
          <option value="MIPS">MIPS</option>
          <option value="8085">Intel 8085</option>
          <option value="6502">6502</option>
          <option value="8086">Intel 8086</option>
        </select>
        <button data-tour="assemble" onClick={() => assemble()} className="btn btn-primary" title="Assemble">
          Assemble
        </button>
        <div className="icon-controls">
          <button
            data-tour="run-pause"
            onClick={isRunning ? pause : run}
            disabled={halted}
            className="btn btn-icon"
            title={isRunning ? "Pause" : "Play"}
          >
            {isRunning ? <PauseIcon /> : <PlayIcon />}
          </button>
          <button
            data-tour="stop"
            onClick={pause}
            className="btn btn-icon"
            title="Stop"
          >
            <StopIcon />
          </button>
          <button
            data-tour="reset"
            onClick={reset}
            className="btn btn-icon"
            title="Reset"
          >
            <ResetIcon />
          </button>
          <button
            data-tour="step-forward"
            onClick={stepForward}
            disabled={halted || !snapshot}
            className="btn btn-icon"
            title="Step Forward"
          >
            <StepForwardIcon />
          </button>
          <button
            data-tour="step-back"
            onClick={stepBack}
            disabled={!canStepBack || isRunning}
            className="btn btn-icon"
            title="Step Back"
          >
            <StepBackIcon />
          </button>
        </div>
      </div>
    </div>
  );
}
