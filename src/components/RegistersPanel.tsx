import { useStore } from "../store";

function formatHex(value: number, digits: number): string {
  const mask = digits === 4 ? 0xFFFF : 0xFFFFFFFF;
  return "0x" + (value & mask).toString(16).padStart(digits, "0").toUpperCase();
}

function RegRow({
  name,
  value,
  nonzero,
  isPc,
}: {
  name: string;
  value: string;
  nonzero: boolean;
  isPc?: boolean;
}) {
  return (
    <div
      className={`reg-row${nonzero ? " nonzero" : ""}${isPc ? " pc-row" : ""}`}
    >
      <span className="reg-name">{name}</span>
      <span className={`reg-value${nonzero ? " nonzero" : ""}`}>{value}</span>
    </div>
  );
}

export function RegistersPanel() {
  const snapshot = useStore((s) => s.snapshot);
  const registerSchema = useStore((s) => s.registerSchema);
  const arch = useStore((s) => s.arch);

  if (!snapshot) {
    return (
      <div className="panel registers-panel">
        <div className="panel-header">
          <h3>Registers</h3>
        </div>
        <div className="panel-placeholder">Loading...</div>
      </div>
    );
  }

  const { state } = snapshot;
  const isLC3 = arch === "LC3";
  const hexDigits = isLC3 ? 4 : 8;

  // LC-3: always render PC + R0-R7 + PSR explicitly (bypass schema)
  if (isLC3) {
    const regs = state.regs ?? [];
    const r = (i: number) => regs[i] ?? 0;
    const psr = r(8);
    const psrStr = `N=${(psr >> 2) & 1} Z=${(psr >> 1) & 1} P=${psr & 1}`;

    return (
      <div className="panel registers-panel">
        <div className="panel-header">
          <h3>Registers</h3>
        </div>
        <div className="registers-grid registers-grid-lc3">
          <RegRow name="PC" value={formatHex(state.pc, 4)} nonzero={state.pc !== 0} isPc />
          {[0, 1, 2, 3, 4, 5, 6, 7].map((i) => (
            <RegRow key={`R${i}`} name={`R${i}`} value={formatHex(r(i), 4)} nonzero={r(i) !== 0} />
          ))}
          <RegRow name="PSR (NZP)" value={psrStr} nonzero={psr !== 0} />
        </div>
      </div>
    );
  }

  // RV32I, MIPS: use schema
  if (!registerSchema) {
    return (
      <div className="panel registers-panel">
        <div className="panel-header">
          <h3>Registers</h3>
        </div>
        <div className="panel-placeholder">Loading...</div>
      </div>
    );
  }

  const { pc_name, reg_names } = registerSchema;
  return (
    <div className="panel registers-panel">
      <div className="panel-header">
        <h3>Registers</h3>
      </div>
      <div className="registers-grid">
        <RegRow
          name={pc_name}
          value={formatHex(state.pc, hexDigits)}
          nonzero={state.pc !== 0}
          isPc
        />
        {reg_names.map((name, i) => {
          const v = state.regs[i] ?? 0;
          const displayVal = formatHex(v, hexDigits);
          return (
            <RegRow key={name} name={name} value={displayVal} nonzero={v !== 0} />
          );
        })}
      </div>
    </div>
  );
}
