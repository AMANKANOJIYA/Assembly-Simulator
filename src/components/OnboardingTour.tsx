import { useEffect, useMemo, useState } from "react";
import { useStore } from "../store";

const ONBOARDING_KEY = "asim-onboarding-v1-complete";

type Step = {
  title: string;
  body: React.ReactNode;
  target?: string; // data-tour="..."
  placement?: "right" | "left" | "top" | "bottom" | "auto";
};

type Rect = { left: number; top: number; width: number; height: number };

function clamp(v: number, min: number, max: number) {
  return Math.max(min, Math.min(max, v));
}

function queryTarget(target?: string): HTMLElement | null {
  if (!target) return null;
  return document.querySelector<HTMLElement>(`[data-tour="${target}"]`);
}

function getRectForTarget(target?: string): Rect | null {
  const el = queryTarget(target);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  if (!isFinite(r.left) || !isFinite(r.top) || r.width <= 0 || r.height <= 0) return null;
  return { left: r.left, top: r.top, width: r.width, height: r.height };
}

function scrollTargetIntoView(target?: string) {
  const el = queryTarget(target);
  if (!el) return;
  try {
    el.scrollIntoView({ block: "center", inline: "center", behavior: "smooth" });
  } catch {
    // ignore
  }
}

export function OnboardingTour() {
  const onboardingOpen = useStore((s) => (s as any).onboardingOpen as boolean);
  const setOnboardingOpen = useStore((s) => (s as any).setOnboardingOpen as (v: boolean) => void);
  const setHelpOpen = useStore((s) => s.setHelpOpen);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const arch = useStore((s) => s.arch);

  const steps: Step[] = useMemo(
    () => [
      {
        title: "Welcome to Assembly Simulator",
        body: (
          <>
            <p>
              This quick tutorial will show you the core workflow: pick an architecture, write or load a sample,
              assemble, then run/step while watching registers, memory, and the pipeline diagram.
            </p>
            <p className="onboard-muted">Tip: press <strong>←</strong>/<strong>→</strong> to navigate, <strong>Esc</strong> to close.</p>
          </>
        ),
        target: "editor",
        placement: "left",
      },
      {
        title: "1) Choose an architecture",
        body: (
          <>
            <p>
              Use the architecture dropdown in the top toolbar (currently <strong>{arch}</strong>).
              Different architectures have different register names, instruction sets, and I/O conventions.
            </p>
            <p className="onboard-muted">
              After changing architecture, use Samples to load an example for that ISA.
            </p>
          </>
        ),
        target: "arch-select",
        placement: "bottom",
      },
      {
        title: "2) Load a sample (or write your own)",
        body: (
          <>
            <p>
              Use <strong>Samples</strong> in the top bar to load example programs, or type directly in the editor.
            </p>
            <p className="onboard-muted">
              Labels use <code>label:</code>. Entry is typically <code>_start:</code> (RV32I/MIPS) or <code>.ORIG</code> (LC-3).
            </p>
          </>
        ),
        target: "samples",
        placement: "bottom",
      },
      {
        title: "3) Assemble, then Run / Step",
        body: (
          <>
            <p>
              Click <strong>Assemble</strong>. If there are errors, they’ll appear in the editor and as a toast.
            </p>
            <p className="onboard-muted">
              Then use <strong>Play</strong> to run or <strong>Step Forward</strong> to execute one instruction at a time.
            </p>
          </>
        ),
        target: "assemble",
        placement: "bottom",
      },
      {
        title: "4) Watch state changes",
        body: (
          <>
            <p>The left side updates live:</p>
            <ul>
              <li><strong>Architecture Diagram</strong> highlights active pipeline stages.</li>
              <li><strong>Registers</strong> show the current CPU state.</li>
              <li><strong>Memory</strong> shows a hex dump (use Jump to address).</li>
              <li><strong>Trace</strong> shows recent pipeline events.</li>
            </ul>
          </>
        ),
        target: "diagram",
        placement: "right",
      },
      {
        title: "5) Program I/O (input & output)",
        body: (
          <>
            <p>
              Some programs request input (syscalls / TRAPs). When that happens, the Runtime Console appears at the bottom.
              Enter a value and click <strong>Send to Program</strong>.
            </p>
            <p className="onboard-muted">Output printed by the program also shows up there.</p>
          </>
        ),
        target: "runtime-console",
        placement: "top",
      },
      {
        title: "6) Save and reopen projects",
        body: (
          <>
            <p>
              Use <strong>New</strong>, <strong>Open</strong>, and <strong>Save</strong> in the File menu. Projects are saved as <code>.asim</code>.
            </p>
            <p className="onboard-muted">
              The file stores your source, selected architecture, memory size, breakpoints, and some settings.
            </p>
          </>
        ),
        target: "file-save",
        placement: "bottom",
      },
      {
        title: "You’re ready",
        body: (
          <>
            <p>
              That’s it. You can reopen this tutorial anytime from Settings → “Start Tutorial”.
            </p>
            <div className="onboard-actions">
              <button
                type="button"
                className="btn btn-small"
                onClick={() => {
                  setOnboardingOpen(false);
                  setSettingsOpen(false);
                  setHelpOpen(true);
                }}
              >
                Open Help
              </button>
            </div>
          </>
        ),
        target: "settings",
        placement: "bottom",
      },
    ],
    [arch, setHelpOpen, setOnboardingOpen, setSettingsOpen]
  );

  const [idx, setIdx] = useState(0);
  const [rect, setRect] = useState<Rect | null>(null);

  const completeAndClose = (markComplete: boolean) => {
    if (markComplete) {
      try {
        localStorage.setItem(ONBOARDING_KEY, "1");
      } catch {
        // ignore
      }
    }
    setOnboardingOpen(false);
  };

  useEffect(() => {
    if (!onboardingOpen) return;
    setIdx(0);
  }, [onboardingOpen]);

  useEffect(() => {
    if (!onboardingOpen) return;
    const s = steps[idx];
    scrollTargetIntoView(s?.target);
    const update = () => setRect(getRectForTarget(s?.target));
    update();
    const t = window.setTimeout(update, 250);
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.clearTimeout(t);
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [idx, onboardingOpen, steps]);

  useEffect(() => {
    if (!onboardingOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        completeAndClose(false);
        return;
      }
      if (e.key === "ArrowRight" || e.key === "Enter") {
        e.preventDefault();
        setIdx((v) => Math.min(steps.length - 1, v + 1));
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setIdx((v) => Math.max(0, v - 1));
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onboardingOpen, steps.length]);

  if (!onboardingOpen) return null;

  const step = steps[idx];
  const isFirst = idx === 0;
  const isLast = idx === steps.length - 1;

  const pad = 10;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const highlight = rect
    ? {
        left: clamp(rect.left - pad, 8, vw - 8),
        top: clamp(rect.top - pad, 8, vh - 8),
        width: clamp(rect.width + pad * 2, 24, vw - 16),
        height: clamp(rect.height + pad * 2, 24, vh - 16),
      }
    : null;

  const targetCx = highlight ? highlight.left + highlight.width / 2 : vw / 2;
  const targetCy = highlight ? highlight.top + highlight.height / 2 : vh / 2;

  const cardW = 380;
  const cardH = 240; // heuristic for placement calculations
  const gap = 14;
  const placement = step.placement ?? "auto";
  const preferred = placement === "auto" ? (highlight && highlight.left < vw * 0.55 ? "right" : "left") : placement;

  let cardLeft = clamp(vw / 2 - cardW / 2, 12, vw - cardW - 12);
  let cardTop = clamp(vh / 2 - cardH / 2, 12, vh - cardH - 12);
  if (highlight) {
    if (preferred === "right") {
      cardLeft = clamp(highlight.left + highlight.width + gap, 12, vw - cardW - 12);
      cardTop = clamp(targetCy - cardH / 2, 12, vh - cardH - 12);
    } else if (preferred === "left") {
      cardLeft = clamp(highlight.left - cardW - gap, 12, vw - cardW - 12);
      cardTop = clamp(targetCy - cardH / 2, 12, vh - cardH - 12);
    } else if (preferred === "top") {
      cardLeft = clamp(targetCx - cardW / 2, 12, vw - cardW - 12);
      cardTop = clamp(highlight.top - cardH - gap, 12, vh - cardH - 12);
    } else if (preferred === "bottom") {
      cardLeft = clamp(targetCx - cardW / 2, 12, vw - cardW - 12);
      cardTop = clamp(highlight.top + highlight.height + gap, 12, vh - cardH - 12);
    }
  }

  const cardCx = cardLeft + cardW / 2;
  const cardCy = cardTop + 38;

  return (
    <div className="onboard-overlay onboard-spotlight" onClick={() => completeAndClose(false)} role="presentation">
      {/* Blur/dim layer (keeps highlighted area clear) */}
      {highlight ? (
        <>
          <div
            className="onboard-spotlight-dim-slice"
            style={{ left: 0, top: 0, width: "100vw", height: highlight.top }}
            aria-hidden
          />
          <div
            className="onboard-spotlight-dim-slice"
            style={{ left: 0, top: highlight.top, width: highlight.left, height: highlight.height }}
            aria-hidden
          />
          <div
            className="onboard-spotlight-dim-slice"
            style={{
              left: highlight.left + highlight.width,
              top: highlight.top,
              width: `calc(100vw - ${highlight.left + highlight.width}px)`,
              height: highlight.height,
            }}
            aria-hidden
          />
          <div
            className="onboard-spotlight-dim-slice"
            style={{ left: 0, top: highlight.top + highlight.height, width: "100vw", height: `calc(100vh - ${highlight.top + highlight.height}px)` }}
            aria-hidden
          />
        </>
      ) : (
        <div className="onboard-spotlight-dim" aria-hidden />
      )}

      {/* Arrow layer */}
      <svg className="onboard-arrow-layer" width="100%" height="100%" aria-hidden>
        {highlight && (
          <>
            <defs>
              <marker id="onboardArrowHead" markerWidth="10" markerHeight="10" refX="8" refY="5" orient="auto">
                <path d="M0,0 L10,5 L0,10 z" fill="rgba(125, 211, 252, 0.95)" />
              </marker>
            </defs>
            <line
              x1={cardCx}
              y1={cardCy}
              x2={targetCx}
              y2={targetCy}
              stroke="rgba(125, 211, 252, 0.95)"
              strokeWidth="2"
              markerEnd="url(#onboardArrowHead)"
            />
          </>
        )}
      </svg>

      {/* Highlight */}
      {highlight && (
        <div
          className="onboard-spotlight-highlight"
          style={{ left: highlight.left, top: highlight.top, width: highlight.width, height: highlight.height }}
          aria-hidden
        />
      )}

      {/* Card */}
      <div
        className="onboard-spotlight-card"
        style={{ left: cardLeft, top: cardTop, width: cardW }}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Onboarding tutorial"
      >
        <div className="onboard-header">
          <div className="onboard-header-left">
            <h2>Tutorial</h2>
            <span className="onboard-step">
              {idx + 1} / {steps.length}
            </span>
          </div>
          <button type="button" className="btn btn-small" onClick={() => completeAndClose(false)}>
            Skip
          </button>
        </div>

        <div className="onboard-body">
          <h3>{step.title}</h3>
          <div className="onboard-content">{step.body}</div>
        </div>

        <div className="onboard-footer">
          <button type="button" className="btn" onClick={() => setIdx((v) => Math.max(0, v - 1))} disabled={isFirst}>
            Back
          </button>
          <div className="onboard-footer-spacer" />
          {!isLast ? (
            <button type="button" className="btn btn-primary" onClick={() => setIdx((v) => Math.min(steps.length - 1, v + 1))}>
              Next
            </button>
          ) : (
            <button type="button" className="btn btn-primary" onClick={() => completeAndClose(true)}>
              Finish
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function hasCompletedOnboarding(): boolean {
  try {
    return localStorage.getItem(ONBOARDING_KEY) === "1";
  } catch {
    return false;
  }
}

