import { useState } from "react";
import { useStore } from "../store";

function parseAddr(raw: string): number | null {
  const s = raw.trim().replace(/^0x/i, "");
  if (!/^[0-9a-f]+$/i.test(s)) return null;
  const n = parseInt(s, 16);
  return isNaN(n) ? null : n;
}

function fmtHex(v: number | undefined, bytes: number): string {
  if (v === undefined) return "??";
  // Read `bytes` bytes little-endian from a single value
  return "0x" + v.toString(16).toUpperCase().padStart(bytes * 2, "0");
}

/** Read 1/2/4 bytes from flat memory array at given address (little-endian) */
function readMemory(memory: number[], addr: number, size: 1 | 2 | 4): number | undefined {
  if (addr < 0 || addr + size - 1 >= memory.length) return undefined;
  let val = 0;
  for (let i = 0; i < size; i++) {
    val |= (memory[addr + i] ?? 0) << (i * 8);
  }
  return val >>> 0;
}

interface WatchEntry {
  id: string;
  label: string;
  addrHex: string;
  size: 1 | 2 | 4;
}

export function WatchPanel() {
  const snapshot  = useStore((s) => s.snapshot);
  const watchList = useStore((s) => s.watchList);
  const addWatch  = useStore((s) => s.addWatch);
  const removeWatch = useStore((s) => s.removeWatch);

  const [addrInput, setAddrInput] = useState("");
  const [labelInput, setLabelInput] = useState("");
  const [sizeInput, setSizeInput] = useState<"1" | "2" | "4">("4");
  const [error, setError] = useState("");

  const memory = snapshot?.memory ?? [];

  // Parse watchList entries: each stored as "label|addrHex|size"
  const entries: WatchEntry[] = watchList.map((w) => {
    const [label, addrHex, size] = w.split("|");
    return { id: w, label: label ?? "", addrHex: addrHex ?? "0", size: (Number(size) as 1 | 2 | 4) || 4 };
  });

  const handleAdd = () => {
    const addr = parseAddr(addrInput);
    if (addr === null) { setError("Invalid hex address"); return; }
    const label = labelInput.trim() || `0x${addr.toString(16).toUpperCase()}`;
    const key = `${label}|${addr.toString(16).toUpperCase()}|${sizeInput}`;
    addWatch(key);
    setAddrInput("");
    setLabelInput("");
    setError("");
  };

  return (
    <div className="panel watch-panel">
      <div className="panel-header">
        <h3 className="panel-title">Watch</h3>
        {entries.length > 0 && <span className="panel-badge">{entries.length}</span>}
      </div>
      <div className="watch-body">
        {/* Add row */}
        <div className="watch-add-row">
          <input
            className="watch-input"
            placeholder="Label (optional)"
            value={labelInput}
            onChange={(e) => setLabelInput(e.target.value)}
          />
          <input
            className="watch-input watch-input--addr"
            placeholder="0x0000"
            value={addrInput}
            onChange={(e) => { setAddrInput(e.target.value); setError(""); }}
            onKeyDown={(e) => e.key === "Enter" && handleAdd()}
          />
          <select
            className="watch-size-select"
            value={sizeInput}
            onChange={(e) => setSizeInput(e.target.value as "1" | "2" | "4")}
            title="Read size"
          >
            <option value="1">1 B</option>
            <option value="2">2 B</option>
            <option value="4">4 B</option>
          </select>
          <button
            type="button"
            className="btn btn-small watch-add-btn"
            onClick={handleAdd}
            title="Add watch"
          >
            +
          </button>
        </div>
        {error && <p className="watch-error">{error}</p>}

        {/* Watch list */}
        {entries.length === 0 ? (
          <div className="watch-empty">
            <p>No addresses being watched</p>
            <p className="watch-empty-hint">Enter a hex address above and press + to start watching</p>
          </div>
        ) : (
          <ul className="watch-list">
            {entries.map(({ id, label, addrHex, size }) => {
              const addr = parseInt(addrHex, 16);
              const raw = readMemory(memory, addr, size);
              const hex = fmtHex(raw, size);
              const decimal = raw !== undefined ? raw.toString(10) : "?";
              return (
                <li key={id} className="watch-item">
                  <div className="watch-item-info">
                    <span className="watch-item-label">{label}</span>
                    <span className="watch-item-addr">0x{addrHex} ({size}B)</span>
                  </div>
                  <div className="watch-item-values">
                    <span className="watch-item-hex">{hex}</span>
                    <span className="watch-item-dec">{decimal}</span>
                  </div>
                  <button
                    type="button"
                    className="watch-remove-btn"
                    onClick={() => removeWatch(id)}
                    title="Remove watch"
                    aria-label={`Remove watch for ${label}`}
                  >
                    ✕
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
