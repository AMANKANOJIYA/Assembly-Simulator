import { useState, useRef, useEffect, useLayoutEffect } from "react";
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
  SAMPLE_8085_SIMPLE,
  SAMPLE_8085_FULL,
  SAMPLE_6502_SIMPLE,
  SAMPLE_6502_FULL,
  SAMPLE_8086_SIMPLE,
  SAMPLE_8086_FULL,
} from "../samples";

function IconNewFile() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="12" y1="18" x2="12" y2="12" />
      <line x1="9" y1="15" x2="15" y2="15" />
    </svg>
  );
}

function IconOpen() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
    </svg>
  );
}

function IconSave() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
      <path d="M17 21v-8H7v8M7 3v5h8" />
    </svg>
  );
}

function IconSamples() {
  return (
    <svg className="activity-bar-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden>
      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
      <path d="M8 7h8M8 11h6" />
    </svg>
  );
}

/** File actions in the activity bar (navigator), not the top navbar */
export function NavigatorFileActions() {
  const saveFile = useStore((s) => s.saveFile);
  const loadFile = useStore((s) => s.loadFile);
  const newFile = useStore((s) => s.newFile);
  const arch = useStore((s) => s.arch);
  const setSource = useStore((s) => s.setSource);
  const [sampleOpen, setSampleOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const samplesBtnRef = useRef<HTMLButtonElement>(null);
  const [flyoutPos, setFlyoutPos] = useState({ top: 0, left: 0 });

  useLayoutEffect(() => {
    if (!sampleOpen || !samplesBtnRef.current) return;
    const r = samplesBtnRef.current.getBoundingClientRect();
    const pad = 8;
    let left = r.right + pad;
    const top = Math.max(8, r.top);
    const vw = typeof window !== "undefined" ? window.innerWidth : 800;
    const flyoutW = 220;
    if (left + flyoutW > vw - 8) {
      left = Math.max(8, r.left - flyoutW - pad);
    }
    setFlyoutPos({ top, left });
  }, [sampleOpen]);

  useEffect(() => {
    if (!sampleOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setSampleOpen(false);
      }
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [sampleOpen]);

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
        : arch === "8085"
          ? [
              { label: "Simple (MVI, ADD)", code: SAMPLE_8085_SIMPLE },
              { label: "Full demo", code: SAMPLE_8085_FULL },
            ]
          : arch === "6502"
            ? [
                { label: "Simple (LDA, ADC)", code: SAMPLE_6502_SIMPLE },
                { label: "Full demo (JSR, RTS)", code: SAMPLE_6502_FULL },
              ]
            : arch === "8086"
              ? [
                  { label: "Simple (MOV, ADD)", code: SAMPLE_8086_SIMPLE },
                  { label: "Full demo", code: SAMPLE_8086_FULL },
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
    <div className="activity-bar-files">
      <button
        data-tour="file-new"
        type="button"
        className="activity-bar-btn activity-bar-btn--icon"
        onClick={newFile}
        title="New file (⌘N)"
        aria-label="New file"
      >
        <span className="activity-bar-icon-wrap" aria-hidden>
          <IconNewFile />
        </span>
        <span className="activity-bar-label">New</span>
      </button>
      <button
        data-tour="file-open"
        type="button"
        className="activity-bar-btn activity-bar-btn--icon"
        onClick={() => loadFile()}
        title="Open .asim file (⌘O)"
        aria-label="Open file"
      >
        <span className="activity-bar-icon-wrap" aria-hidden>
          <IconOpen />
        </span>
        <span className="activity-bar-label">Open</span>
      </button>
      <button
        data-tour="file-save"
        type="button"
        className="activity-bar-btn activity-bar-btn--icon"
        onClick={() => saveFile()}
        title="Save as .asim (⌘S)"
        aria-label="Save file"
      >
        <span className="activity-bar-icon-wrap" aria-hidden>
          <IconSave />
        </span>
        <span className="activity-bar-label">Save</span>
      </button>
      <div className="activity-bar-flyout-wrap" ref={wrapRef}>
        <button
          ref={samplesBtnRef}
          data-tour="samples"
          type="button"
          className={`activity-bar-btn activity-bar-btn--icon${sampleOpen ? " is-active" : ""}`}
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setSampleOpen((o) => !o);
          }}
          title="Load sample program"
          aria-label="Samples"
          aria-expanded={sampleOpen}
        >
          <span className="activity-bar-icon-wrap" aria-hidden>
            <IconSamples />
          </span>
          <span className="activity-bar-label">Samples</span>
        </button>
        {sampleOpen && (
          <div
            className="activity-bar-flyout activity-bar-flyout--fixed"
            role="menu"
            style={{ top: flyoutPos.top, left: flyoutPos.left }}
          >
            {samples.map(({ label, code }) => (
              <button
                key={label}
                type="button"
                role="menuitem"
                className="activity-bar-flyout-item"
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
        )}
      </div>
    </div>
  );
}
