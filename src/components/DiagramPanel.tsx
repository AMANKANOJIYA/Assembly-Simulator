import { useRef, useCallback, useState, useEffect, useMemo } from "react";
import { useStore } from "../store";
import type { TraceEvent } from "../types";
import type { UiBlock } from "../types";
import { COMPACT_VB, EXPANDED_VB, effectiveBlock } from "../diagramLayout";

const VB_W = COMPACT_VB.w;
const VB_H = COMPACT_VB.h;
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

/** Straight orthogonal segments + per-edge lane */
function diagramEdgePath(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  edgeIndex: number,
  edgeCount: number
): string {
  const lane = (edgeIndex - (edgeCount - 1) / 2) * 16;
  if (x2 >= x1) {
    const midX = (x1 + x2) / 2 + lane;
    return `M ${x1} ${y1} L ${midX} ${y1} L ${midX} ${y2} L ${x2} ${y2}`;
  }
  const midY = (y1 + y2) / 2 + lane;
  return `M ${x1} ${y1} L ${x1} ${midY} L ${x2} ${midY} L ${x2} ${y2}`;
}

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
  canvasWidth = VB_W,
  canvasHeight = VB_H,
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
  canvasWidth?: number;
  canvasHeight?: number;
}) {
  const dragRef = useRef<{ id: string; startX: number; startY: number } | null>(null);

  // Keep latest callbacks in refs so stable drag handlers always call the current version
  const onBlockDragRef = useRef(onBlockDrag);
  const onBlockDragEndRef = useRef(onBlockDragEnd);
  useEffect(() => { onBlockDragRef.current = onBlockDrag; }, [onBlockDrag]);
  useEffect(() => { onBlockDragEndRef.current = onBlockDragEnd; }, [onBlockDragEnd]);

  const blocksLaidOut = useMemo(
    () => blocks.map((b) => effectiveBlock(b, expanded)),
    [blocks, expanded]
  );

  const blockById = useMemo(() => {
    const m = new Map<string, UiBlock>();
    blocksLaidOut.forEach((b) => m.set(b.id, b));
    return m;
  }, [blocksLaidOut]);

  const getPos = (b: UiBlock) => {
    const override = blockPositions[b.id];
    return override ? { x: override.x, y: override.y } : { x: b.x, y: b.y };
  };

  // Keep the SVG ref and viewBox in a ref so the stable drag move handler can read them
  const svgRefInternal = useRef(svgRef);
  const viewBoxRef = useRef(viewBox);
  const canvasWRef = useRef(canvasWidth);
  const canvasHRef = useRef(canvasHeight);
  useEffect(() => { svgRefInternal.current = svgRef; }, [svgRef]);
  useEffect(() => { viewBoxRef.current = viewBox; }, [viewBox]);
  useEffect(() => { canvasWRef.current = canvasWidth; }, [canvasWidth]);
  useEffect(() => { canvasHRef.current = canvasHeight; }, [canvasHeight]);

  // Stable handler: created once, reads latest values via refs
  const handleBlockMouseDown = useCallback((e: React.MouseEvent, id: string) => {
    e.preventDefault();
    e.stopPropagation();
    dragRef.current = { id, startX: e.clientX, startY: e.clientY };

    const move = (ev: MouseEvent) => {
      if (!dragRef.current) return;
      const dx = ev.clientX - dragRef.current.startX;
      const dy = ev.clientY - dragRef.current.startY;

      // Pixel → viewBox coordinate conversion using current SVG rect
      const svg = svgRefInternal.current?.current;
      let vdx = dx;
      let vdy = dy;
      if (svg) {
        const rect = svg.getBoundingClientRect();
        const parts = viewBoxRef.current.split(/\s+/).map(Number);
        const vbW = parts[2] || canvasWRef.current;
        const vbH = parts[3] || canvasHRef.current;
        vdx = dx * (vbW / rect.width);
        vdy = dy * (vbH / rect.height);
      }

      onBlockDragRef.current(dragRef.current.id, vdx, vdy);
      dragRef.current = { ...dragRef.current, startX: ev.clientX, startY: ev.clientY };
    };

    const up = () => {
      dragRef.current = null;
      onBlockDragEndRef.current();
      document.removeEventListener("mousemove", move);
      document.removeEventListener("mouseup", up);
    };

    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
  }, []); // stable — no deps needed, all values read via refs

  return (
    <svg
      ref={svgRef}
      viewBox={viewBox}
      className={`diagram-svg ${activeBlocks.size > 0 ? "signals-active" : ""} ${expanded ? "expanded" : ""}`}
      preserveAspectRatio="xMidYMid meet"
    >
      {connections.map((c, i) => {
        const from = blockById.get(c.from);
        const to = blockById.get(c.to);
        if (!from || !to) return null;
        const fp = getPos(from);
        const tp = getPos(to);
        const spread = (i % 7) - 3;
        const yOff = spread * 2.5;
        const x1 = fp.x + from.width;
        const y1 = fp.y + from.height / 2 + yOff;
        const x2 = tp.x;
        const y2 = tp.y + to.height / 2 + yOff;
        const d = diagramEdgePath(x1, y1, x2, y2, i, connections.length);
        const isActive = activeConnections.has(`${c.from}->${c.to}`);
        return (
          <path
            key={`c-${i}`}
            d={d}
            fill="none"
            className={`diagram-conn diagram-conn-expanded ${isActive ? "active signal-moving" : ""}`}
          />
        );
      })}
      {onBackgroundMouseDown && (
        <rect
          x={0}
          y={0}
          width={canvasWidth}
          height={canvasHeight}
          fill="transparent"
          style={{ cursor: "grab" }}
          onMouseDown={onBackgroundMouseDown}
          className="diagram-pan-rect"
        />
      )}
      {blocksLaidOut.map((b) => {
        const pos = getPos(b);
        const inners = BLOCK_INNERS[b.id];
        const isActive = activeBlocks.has(b.id);
        const titleY = expanded ? 16 : 12;
        const innerStartY = expanded ? 30 : 26;
        const lineStep = expanded ? 12 : 0;
        const showLines = expanded && inners && inners.length > 0;
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
              rx={expanded ? 8 : 6}
              className={`diagram-block diagram-block--${b.id} ${isActive ? "active" : ""}`}
            />
            <text
              x={b.width / 2}
              y={titleY}
              textAnchor="middle"
              className="diagram-block-title"
            >
              {b.label}
            </text>
            {showLines
              ? inners.slice(0, 5).map((line, idx) => (
                  <text
                    key={idx}
                    x={b.width / 2}
                    y={innerStartY + idx * lineStep}
                    textAnchor="middle"
                    className="diagram-block-inner diagram-block-inner-line"
                  >
                    {line.length > 28 ? `${line.slice(0, 26)}…` : line}
                  </text>
                ))
              : inners && (
                  <text
                    x={b.width / 2}
                    y={innerStartY}
                    textAnchor="middle"
                    className="diagram-block-inner"
                  >
                    {inners.slice(0, 2).join(" | ")}
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

  // Keep zoom/pan in refs so stable pan handlers read current values
  const zoomRef = useRef(zoom);
  const panXRef = useRef(pan.x);
  const panYRef = useRef(pan.y);
  useEffect(() => { zoomRef.current = zoom; }, [zoom]);
  useEffect(() => { panXRef.current = pan.x; panYRef.current = pan.y; }, [pan]);

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

  const viewBoxW = EXPANDED_VB.w / zoom;
  const viewBoxH = EXPANDED_VB.h / zoom;
  const viewBox = `${pan.x} ${pan.y} ${viewBoxW} ${viewBoxH}`;

  // Keep viewBox dims in refs for stable pan move handler
  const viewBoxWRef = useRef(viewBoxW);
  const viewBoxHRef = useRef(viewBoxH);
  useEffect(() => { viewBoxWRef.current = viewBoxW; }, [viewBoxW]);
  useEffect(() => { viewBoxHRef.current = viewBoxH; }, [viewBoxH]);

  useEffect(() => {
    if (archExpanded) resetBlockPositions();
  }, [archExpanded, resetBlockPositions]);

  const handleZoomIn = () => setZoom((z) => Math.min(MAX_ZOOM, z + ZOOM_STEP));
  const handleZoomOut = () => setZoom((z) => Math.max(MIN_ZOOM, z - ZOOM_STEP));
  const handleResetView = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  // Stable pan handler: created once, reads all values via refs
  const handleBackgroundMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    setIsPanning(true);
    panRef.current = {
      startX: e.clientX,
      startY: e.clientY,
      startPanX: panXRef.current,
      startPanY: panYRef.current,
    };

    const move = (ev: MouseEvent) => {
      if (!panRef.current) return;
      const svg = expandedSvgRef.current;
      if (!svg) return;
      const rect = svg.getBoundingClientRect();
      const dx = (ev.clientX - panRef.current.startX) * (viewBoxWRef.current / rect.width);
      const dy = (ev.clientY - panRef.current.startY) * (viewBoxHRef.current / rect.height);
      setPan({ x: panRef.current.startPanX - dx, y: panRef.current.startPanY - dy });
    };

    const up = () => {
      setIsPanning(false);
      panRef.current = null;
      document.removeEventListener("mousemove", move);
      document.removeEventListener("mouseup", up);
    };

    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
  }, []); // stable — reads all mutable values via refs

  const activeBlocks = new Set<string>();
  const activeConnections = new Set<string>();
  for (const e of traceEvents) {
    EVENT_TO_BLOCKS[e as TraceEvent]?.forEach((b) => activeBlocks.add(b));
    EVENT_TO_CONNECTIONS[e as TraceEvent]?.forEach(
      ([a, b]) => activeConnections.add(`${a}->${b}`)
    );
  }

  const handleBlockDrag = useCallback(
    (id: string, dx: number, dy: number) => {
      const b = uiSchema?.blocks.find((x) => x.id === id);
      if (!b) return;
      const base = effectiveBlock(b, archExpanded);
      const pos = useStore.getState().blockPositions[id];
      const prev = pos ?? { x: base.x, y: base.y };
      setBlockPosition(id, prev.x + dx, prev.y + dy);
    },
    [uiSchema, archExpanded, setBlockPosition]
  );

  const handleBlockDragEnd = useCallback(() => {}, []);

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
      <div className="panel diagram-panel" data-tour="diagram">
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
            canvasWidth={COMPACT_VB.w}
            canvasHeight={COMPACT_VB.h}
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
          onClick={() => {
            setArchExpanded(false);
            handleResetView();
            resetBlockPositions();
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setArchExpanded(false);
              handleResetView();
              resetBlockPositions();
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
                  onClick={() => {
                    setArchExpanded(false);
                    handleResetView();
                    resetBlockPositions();
                  }}
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
                canvasWidth={EXPANDED_VB.w}
                canvasHeight={EXPANDED_VB.h}
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
