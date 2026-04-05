import { useRef, useCallback, useState, useEffect } from "react";
import { useStore } from "../store";
import type { TraceEvent } from "../types";

// ── Stage palette — same as Pipeline Timing Gantt ────────────────────────────
const STAGE_PALETTE = {
  IF:  { label: "IF",  color: "#6366f1", bg: "rgba(99,102,241,0.10)"  },
  ID:  { label: "ID",  color: "#22c55e", bg: "rgba(34,197,94,0.10)"   },
  EX:  { label: "EX",  color: "#f59e0b", bg: "rgba(245,158,11,0.10)"  },
  MEM: { label: "MEM", color: "#ec4899", bg: "rgba(236,72,153,0.10)"  },
  WB:  { label: "WB",  color: "#3b82f6", bg: "rgba(59,130,246,0.10)"  },
} as const;
type StageName = keyof typeof STAGE_PALETTE;

const BLOCK_STAGE: Record<string, StageName> = {
  pc: "IF", im: "IF",
  ir: "ID", control: "ID",
  regfile: "EX", alu: "EX",
  dm: "MEM",
  mux: "WB",
};

const EV_BLOCKS: Record<string, string[]> = {
  FETCH:     ["pc", "im"],
  DECODE:    ["ir", "control"],
  ALU:       ["regfile", "alu"],
  MEM:       ["alu", "dm"],
  REG_WRITE: ["mux", "regfile"],
  HALTED:    ["control"],
};
const EV_WIRES: Record<string, string[]> = {
  FETCH:     ["pc-im", "im-ir"],
  DECODE:    ["ir-ctrl", "ir-rf"],
  ALU:       ["rf-alu-a", "rf-alu-b", "ir-alu"],
  MEM:       ["alu-dm", "dm-mux"],
  REG_WRITE: ["alu-mux", "mux-rf"],
  HALTED:    ["ctrl-pc"],
};

function getActive(events: string[]) {
  const blocks = new Set<string>();
  const wires  = new Set<string>();
  for (const e of events) {
    EV_BLOCKS[e]?.forEach(b => blocks.add(b));
    EV_WIRES[e]?.forEach(w  => wires.add(w));
  }
  return { blocks, wires };
}

// ── Hardware shapes ──────────────────────────────────────────────────────────

function HwRect({ x, y, w, h, rx = 5, rows = 0, id, active, stageColor, label, sublabel }: {
  x: number; y: number; w: number; h: number; rx?: number; rows?: number;
  id: string; active: boolean; stageColor: string; label: string; sublabel?: string;
}) {
  const rowH = rows > 1 ? h / rows : 0;
  return (
    <g className={`hw-block hw-block--${id} ${active ? "hw-block--active" : ""}`}
       style={{ "--hw-color": stageColor } as React.CSSProperties}>
      <rect x={x} y={y} width={w} height={h} rx={rx} className="hw-rect" />
      {active && <rect x={x - 2} y={y - 2} width={w + 4} height={h + 4} rx={rx + 2} className="hw-glow-ring" fill="none" />}
      {rows > 1 && Array.from({ length: rows - 1 }, (_, i) => (
        <line key={i} x1={x + 4} y1={y + rowH * (i + 1)} x2={x + w - 4} y2={y + rowH * (i + 1)} className="hw-row-line" />
      ))}
      <text x={x + w / 2} y={y + h / 2 - (sublabel ? 7 : 0)} textAnchor="middle" dominantBaseline="middle" className="hw-label">{label}</text>
      {sublabel && <text x={x + w / 2} y={y + h / 2 + 9} textAnchor="middle" dominantBaseline="middle" className="hw-sublabel">{sublabel}</text>}
    </g>
  );
}

/** Classic ALU chevron — inputs on left notch, output on right tip */
function HwAlu({ x, y, w, h, active }: { x: number; y: number; w: number; h: number; active: boolean }) {
  const pts = [
    [0, h * 0.12], [w * 0.62, 0],    [w, h * 0.5],
    [w * 0.62, h], [0, h * 0.88],
    [w * 0.28, h * 0.62], [w * 0.28, h * 0.38],
  ].map(([px, py]) => `${x + px},${y + py}`).join(" ");
  return (
    <g className={`hw-block hw-block--alu ${active ? "hw-block--active" : ""}`}
       style={{ "--hw-color": STAGE_PALETTE.EX.color } as React.CSSProperties}>
      {active && <polygon points={pts} className="hw-alu-glow" />}
      <polygon points={pts} className="hw-alu" />
      <text x={x + w * 0.68} y={y + h * 0.5} textAnchor="middle" dominantBaseline="middle" className="hw-label">ALU</text>
    </g>
  );
}

/** MUX trapezoid — wider on inputs (left), narrower on output (right) */
function HwMux({ x, y, w, h, active }: { x: number; y: number; w: number; h: number; active: boolean }) {
  const pts = `${x},${y} ${x + w},${y + h * 0.22} ${x + w},${y + h * 0.78} ${x},${y + h}`;
  return (
    <g className={`hw-block hw-block--mux ${active ? "hw-block--active" : ""}`}
       style={{ "--hw-color": STAGE_PALETTE.WB.color } as React.CSSProperties}>
      {active && <polygon points={pts} className="hw-mux-glow" />}
      <polygon points={pts} className="hw-mux" />
      <text x={x + w * 0.5} y={y + h * 0.5} textAnchor="middle" dominantBaseline="middle" className="hw-label hw-mux-label">MUX</text>
    </g>
  );
}

/** Animated wire with optional label. markerSuffix lets compact/expanded use separate arrowhead defs. */
function HwWire({ d, active, type = "data", label, labelX, labelY, markerSuffix = "" }: {
  d: string; active: boolean; type?: "data" | "control" | "addr";
  label?: string; labelX?: number; labelY?: number; markerSuffix?: string;
}) {
  return (
    <g>
      <path d={d} fill="none"
        className={`hw-wire hw-wire--${type} ${active ? "hw-wire--on" : ""}`}
        markerEnd={`url(#hw-arrow${markerSuffix})`} />
      {active && <path d={d} fill="none" className="hw-wire-flow" />}
      {label && labelX != null && labelY != null && (
        <text x={labelX} y={labelY} textAnchor="middle"
          className={`hw-bus-label ${active ? "hw-bus-label--on" : ""}`}>{label}</text>
      )}
    </g>
  );
}

// ── Compact diagram (510 × 195) ───────────────────────────────────────────────

const C = {
  zones: [
    { key: "IF",  x: 0,   w: 172, ...STAGE_PALETTE.IF  },
    { key: "ID",  x: 172, w: 116, ...STAGE_PALETTE.ID  },
    { key: "EX",  x: 288, w: 116, ...STAGE_PALETTE.EX  },
    { key: "MEM", x: 404, w: 90,  ...STAGE_PALETTE.MEM },
    { key: "WB",  x: 494, w: 16,  ...STAGE_PALETTE.WB  },
  ],
  pc:      { x: 8,   y: 79,  w: 62,  h: 32 },
  im:      { x: 88,  y: 48,  w: 64,  h: 92 },
  ir:      { x: 182, y: 48,  w: 90,  h: 34 },
  regfile: { x: 182, y: 98,  w: 90,  h: 62 },
  alu:     { x: 296, y: 58,  w: 82,  h: 62 },
  dm:      { x: 408, y: 48,  w: 64,  h: 92 },
  mux:     { x: 488, y: 73,  w: 20,  h: 46 },
  control: { x: 8,   y: 158, w: 220, h: 24 },
};
const CW = 510; const CH = 195;

function CompactDiagram({ activeBlocks: ab, activeWires: aw }: { activeBlocks: Set<string>; activeWires: Set<string> }) {
  return (
    <svg viewBox={`0 0 ${CW} ${CH}`} className="diagram-svg hw-diagram-svg" preserveAspectRatio="xMidYMid meet">
      <defs>
        <marker id="hw-arrow" markerWidth="5" markerHeight="5" refX="4.5" refY="2.5" orient="auto">
          <path d="M0,0 L5,2.5 L0,5 Z" className="hw-arrow-head" />
        </marker>
        <filter id="hw-glow-c" x="-40%" y="-40%" width="180%" height="180%">
          <feGaussianBlur stdDeviation="2.5" result="b" /><feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>

      {/* Stage zones */}
      {C.zones.map(z => (
        <g key={z.key}>
          <rect x={z.x} y={0} width={z.w} height={CH - 28} fill={z.bg} />
          <text x={z.x + z.w / 2} y={11} textAnchor="middle" className="hw-stage-label" style={{ fill: z.color }}>{z.label}</text>
        </g>
      ))}

      {/* Wires */}
      <HwWire d={`M${C.pc.x+C.pc.w},${C.pc.y+C.pc.h/2} L${C.im.x},${C.im.y+C.im.h*0.35}`} active={aw.has("pc-im")} type="addr" />
      <HwWire d={`M${C.im.x+C.im.w},${C.im.y+22} L${C.ir.x},${C.ir.y+C.ir.h/2}`} active={aw.has("im-ir")} type="data" />
      <HwWire d={`M${C.ir.x+45},${C.ir.y+C.ir.h} L${C.ir.x+45},${C.regfile.y}`} active={aw.has("ir-rf")} type="addr" />
      <HwWire d={`M${C.regfile.x+C.regfile.w},${C.regfile.y+18} L${C.alu.x+C.alu.w*0.28},${C.alu.y+C.alu.h*0.33}`} active={aw.has("rf-alu-a")} type="data" />
      <HwWire d={`M${C.regfile.x+C.regfile.w},${C.regfile.y+46} L${C.alu.x+C.alu.w*0.28},${C.alu.y+C.alu.h*0.67}`} active={aw.has("rf-alu-b")} type="data" />
      <HwWire d={`M${C.ir.x+C.ir.w},${C.ir.y+C.ir.h/2} L${C.alu.x+C.alu.w*0.28},${C.alu.y+C.alu.h*0.42}`} active={aw.has("ir-alu")} type="control" />
      <HwWire d={`M${C.alu.x+C.alu.w},${C.alu.y+C.alu.h/2} L${C.dm.x},${C.dm.y+28}`} active={aw.has("alu-dm")} type="addr" />
      <HwWire d={`M${C.dm.x+C.dm.w},${C.dm.y+52} L${C.mux.x},${C.mux.y+C.mux.h*0.64}`} active={aw.has("dm-mux")} type="data" />
      <HwWire d={`M${C.alu.x+C.alu.w},${C.alu.y+18} C${C.dm.x+32},${C.alu.y-16} ${C.mux.x+2},${C.mux.y-2} ${C.mux.x},${C.mux.y+C.mux.h*0.25}`} active={aw.has("alu-mux")} type="data" />
      <HwWire d={`M${C.mux.x+C.mux.w},${C.mux.y+C.mux.h/2} L${CW-4},${C.mux.y+C.mux.h/2} L${CW-4},${CH-8} L${C.regfile.x+55},${CH-8} L${C.regfile.x+55},${C.regfile.y+C.regfile.h}`} active={aw.has("mux-rf")} type="data" />
      <HwWire d={`M${C.ir.x+70},${C.ir.y+C.ir.h} L${C.ir.x+70},${CH-36} L${C.control.x+130},${CH-36} L${C.control.x+130},${C.control.y}`} active={aw.has("ir-ctrl")} type="control" />

      {/* Blocks */}
      <HwRect {...C.pc}      id="pc"      active={ab.has("pc")}      stageColor={STAGE_PALETTE.IF.color}  label="PC"        />
      <HwRect {...C.im}      id="im"      active={ab.has("im")}      stageColor={STAGE_PALETTE.IF.color}  label="I-MEM"     rows={5} />
      <HwRect {...C.ir}      id="ir"      active={ab.has("ir")}      stageColor={STAGE_PALETTE.ID.color}  label="IR / Dec"  />
      <HwRect {...C.regfile} id="regfile" active={ab.has("regfile")} stageColor={STAGE_PALETTE.EX.color}  label="RegFile"   sublabel="x0–x31" rows={4} />
      <HwAlu  {...C.alu}     active={ab.has("alu")} />
      <HwRect {...C.dm}      id="dm"      active={ab.has("dm")}      stageColor={STAGE_PALETTE.MEM.color} label="D-MEM"     rows={5} />
      <HwMux  {...C.mux}     active={ab.has("mux")} />
      <HwRect {...C.control} id="control" active={ab.has("control")} stageColor={STAGE_PALETTE.ID.color}  label="Control"   rx={10} />
    </svg>
  );
}

// ── Expanded diagram (1380 × 680) ─────────────────────────────────────────────

const E = {
  W: 1380, H: 680,
  zones: [
    { key: "IF",  x: 0,    w: 308,  ...STAGE_PALETTE.IF  },
    { key: "ID",  x: 308,  w: 242,  ...STAGE_PALETTE.ID  },
    { key: "EX",  x: 550,  w: 258,  ...STAGE_PALETTE.EX  },
    { key: "MEM", x: 808,  w: 240,  ...STAGE_PALETTE.MEM },
    { key: "WB",  x: 1048, w: 332,  ...STAGE_PALETTE.WB  },
  ],
  pc:      { x: 32,   y: 296, w: 112, h: 66 },
  im:      { x: 172,  y: 188, w: 112, h: 224 },
  ir:      { x: 330,  y: 193, w: 178, h: 72  },
  regfile: { x: 323,  y: 338, w: 192, h: 196 },
  alu:     { x: 563,  y: 252, w: 150, h: 112 },
  dm:      { x: 828,  y: 188, w: 112, h: 224 },
  mux:     { x: 1062, y: 294, w: 46,  h: 144 },
  control: { x: 32,   y: 560, w: 478, h: 72  },
};

function ExpandedDiagram({ activeBlocks: ab, activeWires: aw, isPanning, svgRef, viewBox, onBgMouseDown }: {
  activeBlocks: Set<string>; activeWires: Set<string>; isPanning: boolean;
  svgRef: React.RefObject<SVGSVGElement | null>; viewBox: string;
  onBgMouseDown: (e: React.MouseEvent) => void;
}) {
  const r = (b: { x: number; y: number; w: number; h: number }, fy = 0.5) => ({ x: b.x + b.w, y: b.y + b.h * fy });
  const l = (b: { x: number; y: number; w: number; h: number }, fy = 0.5) => ({ x: b.x,       y: b.y + b.h * fy });
  const t = (b: { x: number; y: number; w: number; h: number }, fx = 0.5) => ({ x: b.x + b.w * fx, y: b.y });
  const b = (bk: { x: number; y: number; w: number; h: number }, fx = 0.5) => ({ x: bk.x + bk.w * fx, y: bk.y + bk.h });

  const aluInA = { x: E.alu.x + E.alu.w * 0.28, y: E.alu.y + E.alu.h * 0.28 };
  const aluInB = { x: E.alu.x + E.alu.w * 0.28, y: E.alu.y + E.alu.h * 0.72 };
  const aluOut = r(E.alu);
  const muxIn0 = l(E.mux, 0.28);
  const muxIn1 = l(E.mux, 0.72);
  const muxOut = r(E.mux, 0.5);

  const SF = "-exp"; // marker suffix for expanded

  function PL({ x, y, text, a = "start" }: { x: number; y: number; text: string; a?: "start" | "end" | "middle" }) {
    return <text x={x} y={y} textAnchor={a} className="hw-port-label">{text}</text>;
  }

  return (
    <svg ref={svgRef} viewBox={viewBox}
      className="diagram-svg hw-diagram-svg hw-expanded"
      preserveAspectRatio="xMidYMid meet"
      style={{ cursor: isPanning ? "grabbing" : "default" }}>
      <defs>
        <marker id={`hw-arrow${SF}`} markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto">
          <path d="M0,0 L6,3 L0,6 Z" className="hw-arrow-head" />
        </marker>
        <filter id="hw-glow-e" x="-40%" y="-40%" width="180%" height="180%">
          <feGaussianBlur stdDeviation="4" result="b" /><feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
      </defs>

      {/* Pan background */}
      <rect x={0} y={0} width={E.W} height={E.H} fill="transparent" onMouseDown={onBgMouseDown} style={{ cursor: isPanning ? "grabbing" : "grab" }} />

      {/* Stage zones */}
      {E.zones.map(z => (
        <g key={z.key}>
          <rect x={z.x} y={18} width={z.w} height={E.H - 26} fill={z.bg} />
          <text x={z.x + z.w / 2} y={36} textAnchor="middle" className="hw-stage-label-exp" style={{ fill: z.color }}>{z.label}</text>
          <line x1={z.x + z.w} y1={18} x2={z.x + z.w} y2={E.H - 8} stroke={z.color} strokeWidth={1} strokeDasharray="4 4" opacity={0.25} />
        </g>
      ))}

      {/* ── Wires ──────────────────────────────────────────────────────── */}
      {/* PC → IM */}
      <HwWire markerSuffix={SF} type="addr"
        d={`M${r(E.pc).x},${r(E.pc).y} L${l(E.im, 0.28).x},${l(E.im, 0.28).y}`}
        active={aw.has("pc-im")} label="PC[31:0]" labelX={152} labelY={286} />
      {/* IM → IR */}
      <HwWire markerSuffix={SF} type="data"
        d={`M${r(E.im).x},${r(E.im, 0.24).y} L${l(E.ir).x},${l(E.ir).y}`}
        active={aw.has("im-ir")} label="Instr[31:0]" labelX={282} labelY={224} />
      {/* IR → RegFile (read addrs) */}
      <HwWire markerSuffix={SF} type="addr"
        d={`M${b(E.ir, 0.34).x},${b(E.ir, 0.34).y} L${t(E.regfile, 0.34).x},${t(E.regfile, 0.34).y}`}
        active={aw.has("ir-rf")} label="rs1,rs2" labelX={366} labelY={322} />
      {/* IR → Control */}
      <HwWire markerSuffix={SF} type="control"
        d={`M${b(E.ir, 0.72).x},${b(E.ir, 0.72).y} L${b(E.ir, 0.72).x},${E.control.y} L${E.control.x + E.control.w * 0.42},${E.control.y}`}
        active={aw.has("ir-ctrl")} label="opcode" labelX={445} labelY={524} />
      {/* RegFile → ALU A */}
      <HwWire markerSuffix={SF} type="data"
        d={`M${r(E.regfile, 0.26).x},${r(E.regfile, 0.26).y} L${aluInA.x},${aluInA.y}`}
        active={aw.has("rf-alu-a")} label="A" labelX={540} labelY={374} />
      {/* RegFile → ALU B */}
      <HwWire markerSuffix={SF} type="data"
        d={`M${r(E.regfile, 0.74).x},${r(E.regfile, 0.74).y} L${aluInB.x},${aluInB.y}`}
        active={aw.has("rf-alu-b")} label="B" labelX={540} labelY={452} />
      {/* IR → ALU (Imm sign-ext) */}
      <HwWire markerSuffix={SF} type="control"
        d={`M${r(E.ir).x},${r(E.ir).y} L${aluInA.x},${aluInA.y - 10}`}
        active={aw.has("ir-alu")} label="Imm" labelX={516} labelY={238} />
      {/* Control → ALU ctrl */}
      <HwWire markerSuffix={SF} type="control"
        d={`M${E.control.x + E.control.w * 0.6},${E.control.y} L${E.control.x + E.control.w * 0.6},${aluInB.y + 28} L${aluInB.x},${aluInB.y + 28}`}
        active={aw.has("ir-ctrl")} label="ALUctrl" labelX={612} labelY={542} />
      {/* Control → RegFile */}
      <HwWire markerSuffix={SF} type="control"
        d={`M${E.control.x + E.control.w * 0.33},${E.control.y} L${E.control.x + E.control.w * 0.33},${r(E.regfile, 0.92).y} L${r(E.regfile, 0.92).x},${r(E.regfile, 0.92).y}`}
        active={aw.has("ir-ctrl")} label="RegWr" labelX={282} labelY={542} />
      {/* ALU → DM addr */}
      <HwWire markerSuffix={SF} type="addr"
        d={`M${aluOut.x},${aluOut.y} L${l(E.dm, 0.28).x},${l(E.dm, 0.28).y}`}
        active={aw.has("alu-dm")} label="Addr" labelX={750} labelY={290} />
      {/* Control → DM */}
      <HwWire markerSuffix={SF} type="control"
        d={`M${E.control.x + E.control.w * 0.73},${E.control.y} L${E.control.x + E.control.w * 0.73},${l(E.dm, 0.74).y} L${l(E.dm, 0.74).x},${l(E.dm, 0.74).y}`}
        active={aw.has("ir-ctrl")} label="MemR/W" labelX={808} labelY={542} />
      {/* DM → MUX */}
      <HwWire markerSuffix={SF} type="data"
        d={`M${r(E.dm, 0.5).x},${r(E.dm, 0.5).y} L${muxIn1.x},${muxIn1.y}`}
        active={aw.has("dm-mux")} label="RData" labelX={982} labelY={386} />
      {/* ALU → MUX (bypass over DM) */}
      <HwWire markerSuffix={SF} type="data"
        d={`M${aluOut.x},${aluOut.y - 22} L${muxIn0.x + 2},${aluOut.y - 22} L${muxIn0.x},${muxIn0.y}`}
        active={aw.has("alu-mux")} label="Result" labelX={980} labelY={214} />
      {/* MUX → RegFile (writeback) */}
      <HwWire markerSuffix={SF} type="data"
        d={`M${muxOut.x},${muxOut.y} L${E.W - 28},${muxOut.y} L${E.W - 28},${E.H - 18} L${t(E.regfile, 0.72).x},${E.H - 18} L${t(E.regfile, 0.72).x},${t(E.regfile, 0.72).y}`}
        active={aw.has("mux-rf")} label="WData" labelX={1260} labelY={372} />
      {/* Control → PC */}
      <HwWire markerSuffix={SF} type="control"
        d={`M${E.control.x + E.control.w * 0.11},${E.control.y} L${E.control.x + E.control.w * 0.11},${r(E.pc, 0.84).y} L${r(E.pc, 0.84).x},${r(E.pc, 0.84).y}`}
        active={aw.has("ctrl-pc")} />

      {/* ── Blocks ─────────────────────────────────────────────────────── */}
      <HwRect {...E.pc}      id="pc"      active={ab.has("pc")}      stageColor={STAGE_PALETTE.IF.color}  label="PC"                              />
      <HwRect {...E.im}      id="im"      active={ab.has("im")}      stageColor={STAGE_PALETTE.IF.color}  label="Instruction Memory"  rows={9}    />
      <HwRect {...E.ir}      id="ir"      active={ab.has("ir")}      stageColor={STAGE_PALETTE.ID.color}  label="Instruction Register"            />
      <HwRect {...E.regfile} id="regfile" active={ab.has("regfile")} stageColor={STAGE_PALETTE.EX.color}  label="Register File" sublabel="x0–x31 (32 × 32-bit)" rows={8} />
      <HwAlu  {...E.alu}     active={ab.has("alu")} />
      <HwRect {...E.dm}      id="dm"      active={ab.has("dm")}      stageColor={STAGE_PALETTE.MEM.color} label="Data Memory"          rows={9}    />
      <HwMux  {...E.mux}     active={ab.has("mux")} />
      <HwRect {...E.control} id="control" active={ab.has("control")} stageColor={STAGE_PALETTE.ID.color}  label="Control Unit"         rx={14}     />

      {/* ── IR bit fields ─────────────────────────────────────────────── */}
      {[
        { label: "opcode", x: 0.05 }, { label: "rd",      x: 0.22 },
        { label: "funct3", x: 0.36 }, { label: "rs1",     x: 0.52 },
        { label: "rs2",    x: 0.66 }, { label: "funct7",  x: 0.80 },
      ].map(({ label, x }) => (
        <g key={label}>
          <line x1={E.ir.x + E.ir.w * x} y1={E.ir.y + 22} x2={E.ir.x + E.ir.w * x} y2={E.ir.y + E.ir.h} className="hw-row-line" />
          <text x={E.ir.x + E.ir.w * (x + 0.07)} y={E.ir.y + E.ir.h - 8} textAnchor="middle" className="hw-field-label">{label}</text>
        </g>
      ))}

      {/* ── Port labels ────────────────────────────────────────────────── */}
      <PL x={E.im.x + 5}              y={E.im.y + 32}               text="Addr →"   />
      <PL x={E.im.x + E.im.w - 5}     y={E.im.y + 32}  a="end"      text="Instr[31:0]" />
      <PL x={E.dm.x + 5}              y={E.dm.y + 30}               text="Addr →"   />
      <PL x={E.dm.x + 5}              y={E.dm.y + 56}               text="WData →"  />
      <PL x={E.dm.x + E.dm.w - 5}     y={E.dm.y + 112} a="end"      text="← RData"  />
      <PL x={E.regfile.x + 5}         y={E.regfile.y + 30}          text="A1(rs1)"  />
      <PL x={E.regfile.x + 5}         y={E.regfile.y + 58}          text="A2(rs2)"  />
      <PL x={E.regfile.x + 5}         y={E.regfile.y + 86}          text="A3(rd)"   />
      <PL x={E.regfile.x + 5}         y={E.regfile.y + E.regfile.h - 14} text="WData" />
      <PL x={E.regfile.x + E.regfile.w - 5} y={E.regfile.y + 30}   a="end" text="RD1 →" />
      <PL x={E.regfile.x + E.regfile.w - 5} y={E.regfile.y + 72}   a="end" text="RD2 →" />
      <PL x={E.mux.x + E.mux.w / 2}  y={E.mux.y - 8}  a="middle"   text="0"        />
      <PL x={E.mux.x + E.mux.w / 2}  y={E.mux.y + E.mux.h + 12} a="middle" text="1" />

      {/* Legend */}
      <g transform={`translate(${E.W - 218}, ${E.H - 72})`}>
        <rect x={0} y={0} width={212} height={66} rx={7} className="hw-legend" />
        <text x={10} y={14} className="hw-legend-title">Signal types</text>
        {[
          { type: "data",    label: "Data / operand", y: 28 },
          { type: "addr",    label: "Address",        y: 43 },
          { type: "control", label: "Control signal", y: 58 },
        ].map(({ type, label, y }) => (
          <g key={type}>
            <line x1={10} y1={y - 4} x2={34} y2={y - 4} className={`hw-wire hw-wire--${type}`} markerEnd={`url(#hw-arrow${SF})`} />
            <text x={42} y={y} className="hw-legend-item">{label}</text>
          </g>
        ))}
      </g>
    </svg>
  );
}

// ── DiagramPanel ─────────────────────────────────────────────────────────────

const MIN_ZOOM = 0.4; const MAX_ZOOM = 4; const ZOOM_STEP = 0.15;

export function DiagramPanel() {
  const arch            = useStore(s => s.arch);
  const traceEvents     = useStore(s => s.snapshot?.trace_events ?? []);
  const archExpanded    = useStore(s => s.archExpanded);
  const setArchExpanded = useStore(s => s.setArchExpanded);
  // keep store compat
  useStore(s => s.resetBlockPositions);

  const [zoom, setZoom] = useState(1);
  const [pan,  setPan]  = useState({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);
  const svgRef       = useRef<SVGSVGElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const zoomRef = useRef(zoom);
  const panRef  = useRef(pan);
  useEffect(() => { zoomRef.current = zoom; }, [zoom]);
  useEffect(() => { panRef.current  = pan;  }, [pan]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || !archExpanded) return;
    const fn = (e: WheelEvent) => {
      e.preventDefault();
      setZoom(z => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP))));
    };
    el.addEventListener("wheel", fn, { passive: false });
    return () => el.removeEventListener("wheel", fn);
  }, [archExpanded]);

  useEffect(() => {
    if (!archExpanded) return;
    const fn = (e: KeyboardEvent) => { if (e.key === "Escape") { setArchExpanded(false); setZoom(1); setPan({ x: 0, y: 0 }); } };
    window.addEventListener("keydown", fn);
    return () => window.removeEventListener("keydown", fn);
  }, [archExpanded, setArchExpanded]);

  const handleBgMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault(); e.stopPropagation();
    setIsPanning(true);
    const sx = e.clientX; const sy = e.clientY;
    const sp = panRef.current;
    const move = (ev: MouseEvent) => {
      const svg = svgRef.current;
      if (!svg) return;
      const rect = svg.getBoundingClientRect();
      const vbW = E.W / zoomRef.current;
      const vbH = E.H / zoomRef.current;
      setPan({ x: sp.x - (ev.clientX - sx) * (vbW / rect.width), y: sp.y - (ev.clientY - sy) * (vbH / rect.height) });
    };
    const up = () => { setIsPanning(false); document.removeEventListener("mousemove", move); document.removeEventListener("mouseup", up); };
    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
  }, []);

  const { blocks: ab, wires: aw } = getActive(traceEvents as TraceEvent[]);
  const viewBox = `${pan.x} ${pan.y} ${E.W / zoom} ${E.H / zoom}`;
  const isRV32I = arch === "RV32I";

  // Active event chip
  const lastEvent = traceEvents[traceEvents.length - 1];
  const lastStage: StageName = lastEvent ? (BLOCK_STAGE[(EV_BLOCKS[lastEvent] ?? [])[0]] ?? "IF") : "IF";
  const lastPal = STAGE_PALETTE[lastStage];

  return (
    <>
      <div className="panel diagram-panel" data-tour="diagram">
        <div className="panel-header">
          <h3 className="panel-title">Architecture</h3>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            {isRV32I && lastEvent && (
              <span className="hw-active-chip"
                style={{ background: lastPal.bg, color: lastPal.color, border: `1px solid ${lastPal.color}55` }}>
                {lastEvent}
              </span>
            )}
            <button type="button" className="btn btn-small" onClick={() => setArchExpanded(true)}>⊞ Expand</button>
          </div>
        </div>
        <div className="diagram-container">
          {isRV32I
            ? <CompactDiagram activeBlocks={ab} activeWires={aw} />
            : <div className="diagram-placeholder" style={{ padding: "18px 12px", textAlign: "center", color: "var(--app-fg-muted)", fontSize: "0.8rem" }}>
                Click <strong>Expand</strong> to view the {arch} architecture diagram
              </div>}
        </div>
        <div className="diagram-hint">Expand for full circuit diagram with signal flow</div>
      </div>

      {archExpanded && (
        <div className="arch-expanded-overlay" onClick={() => { setArchExpanded(false); setZoom(1); setPan({ x: 0, y: 0 }); }}>
          <div className="arch-expanded-content hw-expanded-content" onClick={e => e.stopPropagation()}>
            <div className="arch-expanded-header">
              <div>
                <h3 style={{ margin: 0 }}>RV32I — Single-Cycle Datapath</h3>
                <p style={{ margin: "3px 0 0", fontSize: "0.75rem", color: "var(--app-fg-muted)" }}>
                  Active stage highlighted · scroll to zoom · drag to pan
                </p>
              </div>
              <div className="arch-expanded-controls">
                <div className="arch-zoom-controls">
                  <button type="button" className="btn btn-small" onClick={() => setZoom(z => Math.max(MIN_ZOOM, z - ZOOM_STEP))}>−</button>
                  <span className="arch-zoom-value">{Math.round(zoom * 100)}%</span>
                  <button type="button" className="btn btn-small" onClick={() => setZoom(z => Math.min(MAX_ZOOM, z + ZOOM_STEP))}>+</button>
                </div>
                <button type="button" className="btn btn-small" onClick={() => { setZoom(1); setPan({ x: 0, y: 0 }); }}>↺ Reset</button>
                <button type="button" className="btn" onClick={() => { setArchExpanded(false); setZoom(1); setPan({ x: 0, y: 0 }); }}>✕ Close</button>
              </div>
            </div>

            {/* Active event pills */}
            {traceEvents.length > 0 && (
              <div style={{ display: "flex", gap: 6, padding: "6px 18px", flexShrink: 0, background: "var(--app-surface-elevated)", borderBottom: "1px solid var(--app-border-subtle)", alignItems: "center" }}>
                <span style={{ fontSize: "0.7rem", color: "var(--app-fg-dim)" }}>Active:</span>
                {traceEvents.map((ev, i) => {
                  const stage: StageName = (BLOCK_STAGE[(EV_BLOCKS[ev] ?? [])[0]] ?? "IF") as StageName;
                  const pal = STAGE_PALETTE[stage];
                  return (
                    <span key={i} style={{ fontSize: "0.72rem", fontWeight: 700, padding: "2px 9px", borderRadius: 999, background: pal.bg, color: pal.color, border: `1px solid ${pal.color}44` }}>{ev}</span>
                  );
                })}
              </div>
            )}

            <div ref={containerRef} className="arch-expanded-svg arch-zoom-pan-container">
              <ExpandedDiagram
                activeBlocks={ab} activeWires={aw}
                isPanning={isPanning} svgRef={svgRef}
                viewBox={viewBox} onBgMouseDown={handleBgMouseDown} />
            </div>
            <div className="arch-expanded-hint">Scroll to zoom · Drag background to pan</div>
          </div>
        </div>
      )}
    </>
  );
}
