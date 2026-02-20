import type { PanelId } from "../store";

interface PanelWithMoveProps {
  id: PanelId;
  label: string;
  customizeMode: boolean;
  movePanel: (id: PanelId, direction: "up" | "down") => void;
  children: React.ReactNode;
}

export function PanelWithMove({ id, label, customizeMode, movePanel, children }: PanelWithMoveProps) {
  if (!customizeMode) {
    return <>{children}</>;
  }
  return (
    <div className="panel-with-move">
      <div className="panel-move-bar">
        <span className="panel-move-label">{label}</span>
        <div className="panel-move-btns">
          <button
            type="button"
            className="btn btn-icon btn-move"
            onClick={() => movePanel(id, "up")}
            title="Move up"
          >
            ↑
          </button>
          <button
            type="button"
            className="btn btn-icon btn-move"
            onClick={() => movePanel(id, "down")}
            title="Move down"
          >
            ↓
          </button>
        </div>
      </div>
      <div className="panel-move-content">{children}</div>
    </div>
  );
}
