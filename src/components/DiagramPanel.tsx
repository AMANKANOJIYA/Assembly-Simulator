import { useRef, useCallback, useState, useEffect } from "react";
import { useStore } from "../store";
import type { TraceEvent } from "../types";
import type { UiBlock } from "../types";

const VB_W = 450;
const VB_H = 170;
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 4;
const ZOOM_STEP = 0.2;

// Map trace events to inner CPU blocks
const EVENT_TO_BLOCKS: Record<string, string[]> = {
  FETCH: ["pc", "im", "ir"],
  DECODE: ["ir", "control"],
  ALU: ["regfile", "alu"],
  MEM: ["dm"],
  REG_WRITE: ["mux", "regfile"],
  HALTED: ["control"],
};

const EVENT_TO_CONNECTIONS: Record<string, [string, string][]> = {
  FETCH: [["pc", "im"], ["im", "ir"]],
  DECODE: [["ir", "control"], ["ir", "regfile"]],
  ALU: [["regfile", "alu"], ["ir", "alu"]],
  MEM: [["regfile", "dm"], ["dm", "mux"]],
  REG_WRITE: [["alu", "mux"], ["mux", "regfile"]],
  HALTED: [["control", "pc"]],
};

// Inner labels for each block (CPU internals)
const BLOCK_INNERS: Record<string, string[]> = {
  pc: ["Address", "PC_en", "Reset"],
  im: ["Instruction Mem", "Address →"],
  ir: ["Opcode", "Rx", "Ry", "Rz", "Imm", "ALU_Op"],
  regfile: ["x0-x31", "Reg_wr", "Data ←"],
  alu: ["ADD/SUB", "Flags: Z,C"],
  dm: ["Data Mem", "Mem_rd", "Mem_wr"],
  mux: ["Sel", "ALU|Mem|Imm"],
  control: ["CLK", "Opcode", "PC_en", "Reg_wr", "Mem_rd/wr"],
};

function DiagramContent({
  blocks,
  connections,
  activeBlocks,
  activeConnections,
  blockPositions,
  onBlockDrag,
  onBlockDragEnd,
  svgRef,
  viewBox = `0 0 ${VB_W} ${VB_H}`,
  onBackgroundMouseDown,
  expanded = false,
}: {
  blocks: UiBlock[];
  connections: { from: string; to: string }[];
  activeBlocks: Set<string>;
  activeConnections: Set<string>;
  blockPositions: Record<string, { x: number; y: number }>;
  onBlockDrag: (id: string, dx: number, dy: number) => void;
  onBlockDragEnd: () => void;
  svgRef?: React.RefObject<SVGSVGElement | null>;
  viewBox?: string;
  onBackgroundMouseDown?: (e: React.MouseEvent) => void;
  expanded?: boolean;
}) {
  const dragRef = useRef<{ id: string; startX: number; startY: number } | null>(null);

  const getPos = (b: UiBlock) => {
    const override = blockPositions[b.id];
    return override ? { x: override.x, y: override.y } : { x: b.x, y: b.y };
  };

  const pixelToViewBox = useCallback((dx: number, dy: number) => {
    const svg = svgRef?.current;
    if (!svg) return { dx, dy };
    const rect = svg.getBoundingClientRect();
    const [,, vbW, vbH] = viewBox.split(/\s+/).map(Number);
    const scaleX = (vbW || VB_W) / rect.width;
    const scaleY = (vbH || VB_H) / rect.height;
    return { dx: dx * scaleX, dy: dy * scaleY };
  }, [svgRef, viewBox]);

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!dragRef.current) return;
      const dx = e.clientX - dragRef.current.startX;
      const dy = e.clientY - dragRef.current.startY;
      const { dx: vdx, dy: vdy } = pixelToViewBox(dx, dy);
      onBlockDrag(dragRef.current.id, vdx, vdy);
      dragRef.current = { ...dragRef.current, startX: e.clientX, startY: e.clientY };
    },
    [onBlockDrag, pixelToViewBox]
  );

  const handleMouseUp = useCallback(() => {
    dragRef.current = null;
    onBlockDragEnd();
    document.removeEventListener("mousemove", handleMouseMove);
    document.removeEventListener("mouseup", handleMouseUp);
  }, [onBlockDragEnd, handleMouseMove]);

  const handleBlockMouseDown = (e: React.MouseEvent, id: string) => {
    e.preventDefault();
    e.stopPropagation();
    dragRef.current = { id, startX: e.clientX, startY: e.clientY };
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  };

  return (
    <svg
      ref={svgRef}
      viewBox={viewBox}
      className={`diagram-svg ${activeBlocks.size > 0 ? "signals-active" : ""} ${expanded ? "expanded" : ""}`}
      preserveAspectRatio="xMidYMid meet"
    >
      {connections.map((c, i) => {
        const from = blocks.find((b) => b.id === c.from);
        const to = blocks.find((b) => b.id === c.to);
        if (!from || !to) return null;
        const fp = getPos(from);
        const tp = getPos(to);
        const x1 = fp.x + from.width;
        const y1 = fp.y + from.height / 2;
        const x2 = tp.x;
        const y2 = tp.y + to.height / 2;
        const mid = (x1 + x2) / 2;
        const isActive = activeConnections.has(`${c.from}->${c.to}`);
        return (
          <path
            key={`c-${i}`}
            d={`M ${x1} ${y1} C ${mid} ${y1}, ${mid} ${y2}, ${x2} ${y2}`}
            fill="none"
            className={`diagram-conn ${isActive ? "active signal-moving" : ""}`}
          />
        );
      })}
      {onBackgroundMouseDown && (
        <rect
          x={0}
          y={0}
          width={VB_W}
          height={VB_H}
          fill="transparent"
          style={{ cursor: "grab" }}
          onMouseDown={onBackgroundMouseDown}
          className="diagram-pan-rect"
        />
      )}
      {blocks.map((b) => {
        const pos = getPos(b);
        const inners = BLOCK_INNERS[b.id];
        const isActive = activeBlocks.has(b.id);
        return (
          <g
            key={b.id}
            transform={`translate(${pos.x}, ${pos.y})`}
            onMouseDown={(e) => handleBlockMouseDown(e, b.id)}
            className="diagram-block-g"
            style={{ cursor: "grab" }}
          >
            <rect
              width={b.width}
              height={b.height}
              rx={6}
              className={`diagram-block ${isActive ? "active" : ""}`}
            />
            <text
              x={b.width / 2}
              y={12}
              textAnchor="middle"
              className="diagram-block-title"
            >
              {b.label}
            </text>
            {inners && (
              <text
                x={b.width / 2}
                y={26}
                textAnchor="middle"
                className="diagram-block-inner"
              >
                {expanded ? inners.join(" · ") : inners.slice(0, 2).join(" | ")}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

export function DiagramPanel() {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const expandedSvgRef = useRef<SVGSVGElement | null>(null);
  const uiSchema = useStore((s) => s.uiSchema);
  const traceEvents = useStore((s) => s.snapshot?.trace_events ?? []);
  const archExpanded = useStore((s) => s.archExpanded);
  const setArchExpanded = useStore((s) => s.setArchExpanded);
  const blockPositions = useStore((s) => s.blockPositions);
  const setBlockPosition = useStore((s) => s.setBlockPosition);
  const resetBlockPositions = useStore((s) => s.resetBlockPositions);

  const [zoom, setZoom] = useState(1.2);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);
  const panRef = useRef<{ startX: number; startY: number; startPanX: number; startPanY: number } | null>(null);
  const zoomPanContainerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const el = zoomPanContainerRef.current;
    if (!el || !archExpanded) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
      setZoom((z) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z + delta)));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [archExpanded]);

  const viewBoxW = VB_W / zoom;
  const viewBoxH = VB_H / zoom;
  const viewBox = `${pan.x} ${pan.y} ${viewBoxW} ${viewBoxH}`;

  const handleZoomIn = () => setZoom((z) => Math.min(MAX_ZOOM, z + ZOOM_STEP));
  const handleZoomOut = () => setZoom((z) => Math.max(MIN_ZOOM, z - ZOOM_STEP));
  const handleResetView = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  const handleBackgroundMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    setIsPanning(true);
    panRef.current = { startX: e.clientX, startY: e.clientY, startPanX: pan.x, startPanY: pan.y };
    document.addEventListener("mousemove", handlePanMove);
    document.addEventListener("mouseup", handlePanUp);
  }, [pan.x, pan.y]);

  const handlePanMove = useCallback((e: MouseEvent) => {
    if (!panRef.current) return;
    const svg = expandedSvgRef.current;
    if (!svg) return;
    const rect = svg.getBoundingClientRect();
    const dx = (e.clientX - panRef.current.startX) * (viewBoxW / rect.width);
    const dy = (e.clientY - panRef.current.startY) * (viewBoxH / rect.height);
    setPan({ x: panRef.current.startPanX - dx, y: panRef.current.startPanY - dy });
  }, [viewBoxW, viewBoxH]);

  const handlePanUp = useCallback(() => {
    setIsPanning(false);
    panRef.current = null;
    document.removeEventListener("mousemove", handlePanMove);
    document.removeEventListener("mouseup", handlePanUp);
  }, [handlePanMove]);

  const activeBlocks = new Set<string>();
  const activeConnections = new Set<string>();
  for (const e of traceEvents) {
    EVENT_TO_BLOCKS[e as TraceEvent]?.forEach((b) => activeBlocks.add(b));
    EVENT_TO_CONNECTIONS[e as TraceEvent]?.forEach(
      ([a, b]) => activeConnections.add(`${a}->${b}`)
    );
  }

  const handleBlockDrag = (id: string, dx: number, dy: number) => {
    const b = uiSchema?.blocks.find((x) => x.id === id);
    if (!b) return;
    const prev = blockPositions[id] ?? { x: b.x, y: b.y };
    setBlockPosition(id, prev.x + dx, prev.y + dy);
  };

  const handleBlockDragEnd = () => {};

  if (!uiSchema) {
    return (
      <div className="panel diagram-panel">
        <div className="panel-header">
          <h3>Architecture</h3>
        </div>
        <div className="diagram-placeholder">Loading...</div>
      </div>
    );
  }

  return (
    <>
      <div className="panel diagram-panel">
        <div className="panel-header">
          <h3>Architecture (Inner View)</h3>
          <div className="diagram-actions">
            <button
              type="button"
              className="btn btn-small"
              onClick={() => setArchExpanded(true)}
              title="Expand"
            >
              ⊞ Expand
            </button>
            <button
              type="button"
              className="btn btn-small"
              onClick={resetBlockPositions}
              title="Reset layout"
            >
              ↺ Layout
            </button>
          </div>
        </div>
        <div className="diagram-container diagram-draggable">
          <DiagramContent
            blocks={uiSchema.blocks}
            connections={uiSchema.connections}
            activeBlocks={activeBlocks}
            activeConnections={activeConnections}
            blockPositions={blockPositions}
            onBlockDrag={handleBlockDrag}
            onBlockDragEnd={handleBlockDragEnd}
            svgRef={svgRef}
          />
        </div>
        {traceEvents.length > 0 && (
          <div className="trace-badges">
            {traceEvents.map((e, i) => (
              <span key={i} className="trace-badge">
                {e}
              </span>
            ))}
          </div>
        )}
        <div className="diagram-hint">
          Drag blocks to move. Expand for full view.
        </div>
      </div>

      {archExpanded && (
        <div
          className="arch-expanded-overlay"
          onClick={() => { setArchExpanded(false); handleResetView(); }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setArchExpanded(false);
              handleResetView();
            }
          }}
          role="button"
          tabIndex={0}
          aria-label="Close"
        >
          <div
            className="arch-expanded-content"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="arch-expanded-header">
              <h3>CPU Architecture (Inner View)</h3>
              <div className="arch-expanded-controls">
                <div className="arch-zoom-controls">
                  <button type="button" className="btn btn-small" onClick={handleZoomOut} title="Zoom out">−</button>
                  <span className="arch-zoom-value">{Math.round(zoom * 100)}%</span>
                  <button type="button" className="btn btn-small" onClick={handleZoomIn} title="Zoom in">+</button>
                </div>
                <button type="button" className="btn btn-small" onClick={handleResetView} title="Reset view">↺ Reset</button>
                <button
                  type="button"
                  className="btn"
                  onClick={() => { setArchExpanded(false); handleResetView(); }}
                >
                  ✕ Close
                </button>
              </div>
            </div>
            <div
              ref={zoomPanContainerRef}
              className="arch-expanded-svg arch-zoom-pan-container"
              style={{ cursor: isPanning ? "grabbing" : undefined }}
            >
              <DiagramContent
                blocks={uiSchema.blocks}
                connections={uiSchema.connections}
                activeBlocks={activeBlocks}
                activeConnections={activeConnections}
                blockPositions={blockPositions}
                onBlockDrag={handleBlockDrag}
                onBlockDragEnd={handleBlockDragEnd}
                svgRef={expandedSvgRef}
                viewBox={viewBox}
                onBackgroundMouseDown={handleBackgroundMouseDown}
                expanded
              />
            </div>
            <div className="arch-expanded-hint">
              Scroll to zoom • Drag background to pan • Drag blocks to rearrange
            </div>
          </div>
        </div>
      )}
    </>
  );
}
