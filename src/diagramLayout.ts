import type { UiBlock } from "./types";

/** Compact diagram (toolbar preview) — matches backend schema units */
export const COMPACT_VB = { w: 450, h: 170 } as const;

/**
 * Expanded modal: larger canvas + non-overlapping block positions (RISC-V–style 8-block pipeline).
 * Generic ISAs fall back to scaled coordinates in `getExpandedBlock`.
 */
/** Larger modal canvas — keep in sync with `.arch-expanded-*` min sizes in CSS */
export const EXPANDED_VB = { w: 1080, h: 500 } as const;

/** Wider/taller boxes + more gap so edges don’t stack on the same pixel */
const EXPANDED_OVERRIDES: Record<string, { x: number; y: number; width: number; height: number }> = {
  pc: { x: 56, y: 192, width: 96, height: 56 },
  im: { x: 200, y: 192, width: 118, height: 56 },
  ir: { x: 364, y: 52, width: 110, height: 60 },
  regfile: { x: 364, y: 232, width: 130, height: 64 },
  alu: { x: 580, y: 142, width: 110, height: 64 },
  dm: { x: 580, y: 288, width: 124, height: 64 },
  mux: { x: 800, y: 232, width: 102, height: 64 },
  control: { x: 56, y: 348, width: 260, height: 80 },
};

export function getExpandedBlock(b: UiBlock): UiBlock {
  const o = EXPANDED_OVERRIDES[b.id];
  if (o) {
    return { ...b, x: o.x, y: o.y, width: o.width, height: o.height };
  }
  // Generic: scale from compact canvas into expanded (more air between nodes)
  const sx = EXPANDED_VB.w / COMPACT_VB.w;
  const sy = EXPANDED_VB.h / COMPACT_VB.h;
  return {
    ...b,
    x: b.x * sx * 0.92 + 24,
    y: b.y * sy * 0.92 + 20,
    width: Math.max(b.width * 1.15, 72),
    height: Math.max(b.height * 1.12, 48),
  };
}

export function effectiveBlock(b: UiBlock, expanded: boolean): UiBlock {
  return expanded ? getExpandedBlock(b) : b;
}
