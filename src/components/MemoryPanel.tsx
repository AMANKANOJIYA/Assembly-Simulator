import { useState, useMemo, useRef, useEffect } from "react";
import { useStore } from "../store";
import { invoke } from "@tauri-apps/api/core";

const BYTES_PER_ROW = 16;
const CHUNK_VIEW_THRESHOLD = 64 * 1024; // 64 KB - show chunks above this
const CHUNK_SIZE = 64 * 1024; // 64 KB per chunk

const MEMORY_SIZE_PRESETS = [
  { value: 4096, label: "4 KB" },
  { value: 8192, label: "8 KB" },
  { value: 16384, label: "16 KB" },
  { value: 32768, label: "32 KB" },
  { value: 65536, label: "64 KB" },
  { value: 131072, label: "128 KB" },
  { value: 262144, label: "256 KB" },
  { value: 524288, label: "512 KB" },
  { value: 1048576, label: "1 MB" },
];

export function MemoryPanel() {
  const snapshot = useStore((s) => s.snapshot);
  const memorySize = useStore((s) => s.memorySize);
  const setMemorySize = useStore((s) => s.setMemorySize);
  const refreshState = useStore((s) => s.refreshState);
  const setToast = useStore((s) => s.setToast);
  const [jumpAddr, setJumpAddr] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [expandedChunk, setExpandedChunk] = useState<number | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const jumpScrollTimerRef = useRef<number | null>(null);

  // Clear any pending scroll timer on unmount
  useEffect(() => {
    return () => {
      if (jumpScrollTimerRef.current != null) {
        clearTimeout(jumpScrollTimerRef.current);
      }
    };
  }, []);

  const memory = snapshot?.memory ?? [];
  const size = snapshot?.memory_size ?? memorySize;
  const pc = snapshot?.state.pc;
  const useChunkView = size > CHUNK_VIEW_THRESHOLD;

  const rowContainsPc = (rowAddr: number, byteCount: number) =>
    pc != null && pc >= rowAddr && pc < rowAddr + byteCount;

  const handleMemorySizeChange = async (newSize: number) => {
    if (newSize < 4096 || newSize > 16 * 1024 * 1024) return;
    try {
      setMemorySize(newSize);
      await invoke("set_memory_size", { size: newSize });
      await refreshState();
      setToast({ message: `Memory set to ${(newSize / 1024).toFixed(0)} KB`, type: "info" });
      setSettingsOpen(false);
      setExpandedChunk(null);
    } catch (e) {
      setToast({ message: String(e), type: "error" });
    }
  };

  const chunks = useMemo(() => {
    if (!useChunkView) return [];
    const list: { start: number; end: number; size: number }[] = [];
    for (let start = 0; start < size; start += CHUNK_SIZE) {
      const end = Math.min(start + CHUNK_SIZE, size) - 1;
      list.push({ start, end, size: end - start + 1 });
    }
    return list;
  }, [useChunkView, size]);

  const rows = useMemo(() => {
    const r: { addr: number; bytes: number[] }[] = [];
    let baseAddr = 0;
    let memSlice = memory;
    if (useChunkView && expandedChunk !== null) {
      const chunk = chunks[expandedChunk];
      if (chunk) {
        baseAddr = chunk.start;
        memSlice = memory.slice(chunk.start, chunk.end + 1);
      }
    }
    for (let a = 0; a < memSlice.length; a += BYTES_PER_ROW) {
      const rowAddr = baseAddr + a;
      const row: number[] = [];
      for (let i = 0; i < BYTES_PER_ROW && a + i < memSlice.length; i++) {
        row.push(memSlice[a + i] ?? 0);
      }
      r.push({ addr: rowAddr, bytes: row });
    }
    return r;
  }, [memory, size, useChunkView, expandedChunk, chunks]);

  const handleJump = () => {
    const parsed = parseInt(jumpAddr, 16);
    if (!isNaN(parsed) && parsed >= 0 && parsed < size) {
      setJumpAddr("");
      if (useChunkView) {
        const chunkIdx = Math.floor(parsed / CHUNK_SIZE);
        setExpandedChunk(chunkIdx);
        if (jumpScrollTimerRef.current != null) clearTimeout(jumpScrollTimerRef.current);
        jumpScrollTimerRef.current = window.setTimeout(() => {
          jumpScrollTimerRef.current = null;
          const rowAddr = Math.floor(parsed / BYTES_PER_ROW) * BYTES_PER_ROW;
          const rowEl = scrollContainerRef.current?.querySelector(`[data-addr="${rowAddr}"]`);
          rowEl?.scrollIntoView({ behavior: "smooth", block: "center" });
        }, 50);
      } else {
        const rowAddr = Math.floor(parsed / BYTES_PER_ROW) * BYTES_PER_ROW;
        const rowEl = scrollContainerRef.current?.querySelector(`[data-addr="${rowAddr}"]`);
        rowEl?.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }
  };

  return (
    <div className="panel memory-panel" data-tour="memory">
      <div className="panel-header">
        <h3>Memory</h3>
        <div className="memory-toolbar">
          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            className="btn btn-icon memory-settings-btn"
            title="Memory settings"
            aria-label="Memory settings"
          >
            ⚙
          </button>
          <input
            type="text"
            placeholder="Jump to 0x..."
            value={jumpAddr}
            onChange={(e) => setJumpAddr(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleJump()}
            className="jump-input"
          />
          <button data-tour="memory-jump" onClick={handleJump} className="btn btn-small">
            Jump
          </button>
        </div>
      </div>
      <div ref={scrollContainerRef} className="memory-hex memory-grid">
        {useChunkView && expandedChunk === null ? (
          <div className="memory-chunk-list">
            <p className="memory-chunk-hint">
              Memory is {(size / 1024).toFixed(0)} KB. Open a chunk to view contents:
            </p>
            <div className="memory-chunk-grid">
              {chunks.map((chunk, idx) => (
                <div key={chunk.start} className="memory-chunk-card">
                  <span className="memory-chunk-range">
                    0x{chunk.start.toString(16).padStart(8, "0")} — 0x
                    {chunk.end.toString(16).padStart(8, "0")}
                  </span>
                  <span className="memory-chunk-size">{(chunk.size / 1024).toFixed(0)} KB</span>
                  <button
                    type="button"
                    className="btn btn-small memory-chunk-open"
                    onClick={() => setExpandedChunk(idx)}
                  >
                    Open
                  </button>
                </div>
              ))}
            </div>
          </div>
        ) : useChunkView && expandedChunk !== null ? (
          <>
            <div className="memory-chunk-bar">
              <button
                type="button"
                className="btn btn-small memory-chunk-back"
                onClick={() => setExpandedChunk(null)}
              >
                ← All chunks
              </button>
              <span className="memory-chunk-current">
                Viewing: 0x{chunks[expandedChunk]?.start.toString(16).padStart(8, "0")} — 0x
                {chunks[expandedChunk]?.end.toString(16).padStart(8, "0")}
              </span>
            </div>
            <div className="memory-grid-head" aria-hidden>
              <span className="mem-h-addr">Address</span>
              {Array.from({ length: BYTES_PER_ROW }, (_, i) => (
                <span key={i} className="mem-h-byte">
                  +{i.toString(16).toUpperCase()}
                </span>
              ))}
            </div>
            {rows.map(({ addr, bytes }) => (
              <div
                key={addr}
                className={`memory-row memory-grid-row${rowContainsPc(addr, bytes.length) ? " memory-row--pc" : ""}`}
                data-addr={addr}
              >
                <span className="mem-addr">0x{addr.toString(16).padStart(8, "0")}</span>
                {bytes.map((b, i) => (
                  <span
                    key={i}
                    className={`mem-byte${b !== 0 ? " nonzero" : ""}${pc === addr + i ? " mem-byte--pc" : ""}`}
                  >
                    {b.toString(16).padStart(2, "0")}
                  </span>
                ))}
              </div>
            ))}
          </>
        ) : (
          <>
            {rows.length > 0 && (
              <div className="memory-grid-head" aria-hidden>
                <span className="mem-h-addr">Address</span>
                {Array.from({ length: BYTES_PER_ROW }, (_, i) => (
                  <span key={i} className="mem-h-byte">
                    +{i.toString(16).toUpperCase()}
                  </span>
                ))}
              </div>
            )}
            {rows.map(({ addr, bytes }) => (
              <div
                key={addr}
                className={`memory-row memory-grid-row${rowContainsPc(addr, bytes.length) ? " memory-row--pc" : ""}`}
                data-addr={addr}
              >
                <span className="mem-addr">0x{addr.toString(16).padStart(8, "0")}</span>
                {bytes.map((b, i) => (
                  <span
                    key={i}
                    className={`mem-byte${b !== 0 ? " nonzero" : ""}${pc === addr + i ? " mem-byte--pc" : ""}`}
                  >
                    {b.toString(16).padStart(2, "0")}
                  </span>
                ))}
              </div>
            ))}
          </>
        )}
      </div>

      {settingsOpen && (
        <div
          className="memory-settings-overlay"
          onClick={() => setSettingsOpen(false)}
          role="presentation"
        >
          <div
            className="memory-settings-popup"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-label="Memory settings"
          >
            <h3>Memory Settings</h3>
            <label className="memory-settings-field">
              <span>Size:</span>
              <select
                value={
                  MEMORY_SIZE_PRESETS.some((p) => p.value === memorySize)
                    ? memorySize
                    : MEMORY_SIZE_PRESETS[0].value
                }
                onChange={(e) => handleMemorySizeChange(Number(e.target.value))}
                className="memory-size-select"
              >
                {MEMORY_SIZE_PRESETS.map((p) => (
                  <option key={p.value} value={p.value}>
                    {p.label}
                  </option>
                ))}
              </select>
            </label>
            <p className="memory-settings-hint">
              Memory &gt; 64 KB is shown as chunks. Click &quot;Open&quot; on a chunk to view its contents.
            </p>
            <button
              type="button"
              className="btn btn-small"
              onClick={() => setSettingsOpen(false)}
            >
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
