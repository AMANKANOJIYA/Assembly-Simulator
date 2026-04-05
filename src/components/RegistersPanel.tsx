import { useStore } from "../store";

function formatHex(value: number, digits: number): string {
  const mask = digits === 2 ? 0xFF : digits === 4 ? 0xFFFF : 0xFFFFFFFF;
  return "0x" + (value & mask).toString(16).padStart(digits, "0").toUpperCase();
}

/** Hero display for the program counter */
function PcHero({ label, value, changed }: { label: string; value: string; changed: boolean }) {
  return (
    <div className={`reg-pc-hero${changed ? " reg-pc-hero--changed" : ""}`}>
      <span className="reg-pc-hero-label">{label}</span>
      <span className="reg-pc-hero-value">{value}</span>
      {changed && <span className="reg-changed-dot" title="Changed this step" aria-hidden />}
    </div>
  );
}

function RegCell({
  name,
  value,
  nonzero,
  changed,
}: {
  name: string;
  value: string;
  nonzero: boolean;
  changed: boolean;
}) {
  return (
    <div className={`reg-cell${nonzero ? " nonzero" : ""}${changed ? " reg-cell--changed" : ""}`}>
      <span className="reg-name">{name}</span>
      <span className="reg-value">{value}</span>
      {changed && <span className="reg-changed-dot" title="Changed this step" aria-hidden />}
    </div>
  );
}

function PanelShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="panel registers-panel" data-tour="registers">
      <div className="panel-header">
        <h3 className="panel-title">Registers</h3>
        <span className="panel-header-hint">● = changed</span>
      </div>
      {children}
    </div>
  );
}

export function RegistersPanel() {
  const snapshot       = useStore((s) => s.snapshot);
  const prevRegs       = useStore((s) => s.prevRegs);
  const registerSchema = useStore((s) => s.registerSchema);
  const arch           = useStore((s) => s.arch);

  if (!snapshot) {
    return (
      <PanelShell>
        <div className="panel-placeholder">Assemble to load registers</div>
      </PanelShell>
    );
  }

  const { state } = snapshot;
  const isLC3    = arch === "LC3";
  const is16bit  = arch === "8086" || arch === "8085" || arch === "6502" || isLC3;
  const hexDigits = is16bit ? 4 : 8;

  const pcChanged = prevRegs != null && state.pc !== (snapshot.state.pc); // always false — use prev snapshot
  void pcChanged; // pc change handled below via prevRegs comparison

  // LC-3: fixed layout — PC + R0-R7 + PSR
  if (isLC3) {
    const regs = state.regs ?? [];
    const prev = prevRegs ?? [];
    const r = (i: number) => regs[i] ?? 0;
    const p = (i: number) => prev[i] ?? 0;
    const psr = r(8);
    const psrStr = `N=${(psr >> 2) & 1} Z=${(psr >> 1) & 1} P=${psr & 1}`;

    return (
      <PanelShell>
        <div className="registers-body">
          <PcHero label="PC" value={formatHex(state.pc, 4)} changed={prevRegs != null} />
          <div className="registers-grid">
            {[0, 1, 2, 3, 4, 5, 6, 7].map((i) => (
              <RegCell
                key={`R${i}`}
                name={`R${i}`}
                value={formatHex(r(i), 4)}
                nonzero={r(i) !== 0}
                changed={prevRegs != null && r(i) !== p(i)}
              />
            ))}
          </div>
          <div className="registers-grid registers-grid--wide">
            <RegCell
              name="PSR (NZP)"
              value={psrStr}
              nonzero={psr !== 0}
              changed={prevRegs != null && r(8) !== p(8)}
            />
          </div>
        </div>
      </PanelShell>
    );
  }

  if (!registerSchema) {
    return (
      <PanelShell>
        <div className="panel-placeholder">Loading schema…</div>
      </PanelShell>
    );
  }

  const { pc_name, reg_names } = registerSchema;
  const prev = prevRegs ?? [];

  return (
    <PanelShell>
      <div className="registers-body">
        <PcHero label={pc_name} value={formatHex(state.pc, hexDigits)} changed={prevRegs != null} />
        <div className="registers-grid">
          {reg_names.map((name, i) => {
            const v = state.regs[i] ?? 0;
            const p = prev[i] ?? 0;
            return (
              <RegCell
                key={name}
                name={name}
                value={formatHex(v, hexDigits)}
                nonzero={v !== 0}
                changed={prevRegs != null && v !== p}
              />
            );
          })}
        </div>
      </div>
    </PanelShell>
  );
}
