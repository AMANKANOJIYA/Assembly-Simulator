import { useState } from "react";
import { useStore } from "../store";
import {
  SAMPLE_REGISTERS,
  SAMPLE_MEMORY,
  SAMPLE_BRANCH,
  SAMPLE_READ_PRINT,
  SAMPLE_PRINT,
  SAMPLE_FULL,
  SAMPLE_LC3_SIMPLE,
  SAMPLE_LC3_BRANCH,
  SAMPLE_LC3_FULL,
  SAMPLE_MIPS_SIMPLE,
  SAMPLE_MIPS_BRANCH,
  SAMPLE_MIPS_FULL,
} from "../samples";

export function FileMenu() {
  const saveFile = useStore((s) => s.saveFile);
  const loadFile = useStore((s) => s.loadFile);
  const newFile = useStore((s) => s.newFile);
  const filePath = useStore((s) => s.filePath);
  const arch = useStore((s) => s.arch);
  const setSource = useStore((s) => s.setSource);
  const [sampleOpen, setSampleOpen] = useState(false);

  const samples =
    arch === "LC3"
      ? [
          { label: "Simple (ADD)", code: SAMPLE_LC3_SIMPLE },
          { label: "Branch", code: SAMPLE_LC3_BRANCH },
          { label: "Full demo", code: SAMPLE_LC3_FULL },
        ]
      : arch === "MIPS"
        ? [
            { label: "Simple (add)", code: SAMPLE_MIPS_SIMPLE },
            { label: "Branch", code: SAMPLE_MIPS_BRANCH },
            { label: "Full demo", code: SAMPLE_MIPS_FULL },
          ]
        : [
            { label: "Registers", code: SAMPLE_REGISTERS },
            { label: "Memory", code: SAMPLE_MEMORY },
            { label: "Branch", code: SAMPLE_BRANCH },
            { label: "Read & Print (trap)", code: SAMPLE_READ_PRINT },
            { label: "Print", code: SAMPLE_PRINT },
            { label: "Full demo", code: SAMPLE_FULL },
          ];

  return (
    <div className="file-menu">
      <button type="button" className="btn btn-small" onClick={newFile} title="New file (Ctrl+N)">
        New
      </button>
      <button type="button" className="btn btn-small" onClick={() => loadFile()} title="Open .asim file (Ctrl+O)">
        Open
      </button>
      <button type="button" className="btn btn-small" onClick={() => saveFile()} title="Save as .asim (Ctrl+S)">
        Save
      </button>
      <div className="sample-dropdown">
        <button
          type="button"
          className="btn btn-small"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setSampleOpen((o) => !o);
          }}
          title="Load sample program"
        >
          Samples ▼
        </button>
        {sampleOpen && (
          <>
            <div className="sample-backdrop" onClick={() => setSampleOpen(false)} aria-hidden="true" />
            <div className="sample-menu">
              {samples.map(({ label, code }) => (
                <button
                  key={label}
                  type="button"
                  className="sample-item"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    setSource(code);
                    setSampleOpen(false);
                  }}
                >
                  {label}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
      {filePath && (
        <span className="file-path" title={filePath}>
          {filePath.split(/[/\\]/).pop()}
        </span>
      )}
    </div>
  );
}
