import { useEffect } from "react";
import { useStore } from "../store";

const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
const mod = isMac ? "⌘" : "Ctrl";

interface Shortcut {
  keys: string[];
  description: string;
}

const SECTIONS: { title: string; items: Shortcut[] }[] = [
  {
    title: "File",
    items: [
      { keys: [mod, "N"], description: "New file" },
      { keys: [mod, "O"], description: "Open .asim project" },
      { keys: [mod, "S"], description: "Save .asim project" },
    ],
  },
  {
    title: "Editor",
    items: [
      { keys: [mod, "F"], description: "Find / replace in editor" },
      { keys: [mod, "Z"], description: "Undo" },
      { keys: [mod, "Shift", "Z"], description: "Redo" },
      { keys: [mod, "/"], description: "Toggle line comment" },
      { keys: ["Tab"], description: "Indent selection" },
    ],
  },
  {
    title: "Simulator",
    items: [
      { keys: [mod, "Enter"], description: "Assemble" },
      { keys: ["Space"], description: "Run / Pause" },
      { keys: ["F10"], description: "Step forward" },
      { keys: ["F9"], description: "Step back" },
    ],
  },
  {
    title: "UI",
    items: [
      { keys: ["?"], description: "Open keyboard shortcuts" },
      { keys: ["Escape"], description: "Close any modal / dropdown" },
    ],
  },
];

function Key({ k }: { k: string }) {
  return <kbd className="shortcut-key">{k}</kbd>;
}

export function KeyboardShortcutsModal() {
  const shortcutsOpen = useStore((s) => s.shortcutsOpen);
  const setShortcutsOpen = useStore((s) => s.setShortcutsOpen);

  useEffect(() => {
    if (!shortcutsOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShortcutsOpen(false);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [shortcutsOpen, setShortcutsOpen]);

  if (!shortcutsOpen) return null;

  return (
    <div
      className="modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
      onClick={(e) => e.target === e.currentTarget && setShortcutsOpen(false)}
    >
      <div className="modal-panel shortcuts-modal">
        <div className="modal-header">
          <h2 className="modal-title">Keyboard Shortcuts</h2>
          <button
            type="button"
            className="modal-close-btn"
            aria-label="Close"
            onClick={() => setShortcutsOpen(false)}
          >
            ✕
          </button>
        </div>
        <div className="shortcuts-grid">
          {SECTIONS.map((section) => (
            <div key={section.title} className="shortcuts-section">
              <h3 className="shortcuts-section-title">{section.title}</h3>
              <ul className="shortcuts-list">
                {section.items.map((item) => (
                  <li key={item.description} className="shortcut-row">
                    <span className="shortcut-keys">
                      {item.keys.map((k, i) => (
                        <span key={k}>
                          {i > 0 && <span className="shortcut-plus">+</span>}
                          <Key k={k} />
                        </span>
                      ))}
                    </span>
                    <span className="shortcut-desc">{item.description}</span>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
