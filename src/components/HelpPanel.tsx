import { useStore } from "../store";
import { invoke } from "@tauri-apps/api/core";
import { getDefaultSample } from "../samples";

export function HelpPanel() {
  const helpOpen = useStore((s) => s.helpOpen);
  const setHelpOpen = useStore((s) => s.setHelpOpen);
  const setOnboardingOpen = useStore((s) => (s as any).setOnboardingOpen as (v: boolean) => void);
  const setToast = useStore((s) => s.setToast);
  const arch = useStore((s) => s.arch);

  const handleOpenLink = async (url: string) => {
    try {
      await invoke("plugin:opener|open_url", { url });
    } catch (e) {
      setToast({ message: `Could not open link: ${e}`, type: "error" });
    }
  };

  const sampleCode = getDefaultSample(arch);

  const regsDesc =
    arch === "LC3"
      ? "Displays PC and R0–R7, plus PSR (condition codes N/Z/P)."
      : arch === "MIPS"
        ? "Displays PC and $0–$31 with ABI names (zero, v0, a0, t0, etc.)."
        : "Displays PC and x0–x31 with ABI names (zero, ra, sp, etc.).";

  if (!helpOpen) return null;

  return (
    <div
      className="help-overlay"
      onClick={() => setHelpOpen(false)}
      role="presentation"
    >
      <div
        className="help-popup"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Help"
      >
        <div className="help-header">
          <div className="help-header-brand">
            <img className="app-logo" src="/logo.png" alt="" decoding="async" />
            <h2>Help — Assembly Simulator</h2>
          </div>
          <div className="settings-samples">
            <button
              type="button"
              className="btn btn-small"
              onClick={() => {
                setHelpOpen(false);
                setOnboardingOpen(true);
              }}
            >
              Start Tutorial
            </button>
            <button
              type="button"
              className="btn btn-small"
              onClick={() => setHelpOpen(false)}
            >
              Close
            </button>
          </div>
        </div>
        <div className="help-body">
          <section>
            <h3>1. How to Use the Application</h3>
            <ol>
              <li>
                <strong>Write assembly</strong> in the code editor (right side). Use {arch} syntax.
              </li>
              <li>
                <strong>Assemble</strong> — Click <code>Assemble</code> to compile your code for the selected architecture. Errors appear in the editor and as toasts.
              </li>
              <li>
                <strong>Run / Step</strong> — Use <span className="help-icon">▶</span> to run continuously, or <span className="help-icon">▷</span> to step one instruction at a time.
              </li>
              <li>
                <strong>Pause / Stop</strong> — <span className="help-icon">⏸</span> pauses execution; <span className="help-icon">⏹</span> stops it.
              </li>
              <li>
                <strong>Reset</strong> — <span className="help-icon">↺</span> resets CPU and memory, reloads the program.
              </li>
              <li>
                <strong>Step Back</strong> — <span className="help-icon">▢</span> undoes the last instruction (registers + memory).
              </li>
              <li>
                <strong>Speed</strong> — Adjust the slider (10–500 ms) to control run speed.
              </li>
            </ol>
          </section>

          <section>
            <h3>2. Panels and Layout</h3>
            <ul>
              <li>
                <strong>Architecture Diagram</strong> — Shows PC, Fetch, Decode, ALU, Memory, RegFile. Active stage is highlighted. Drag blocks to rearrange in Customize mode. Scroll to zoom, drag background to pan.
              </li>
              <li>
                <strong>Registers</strong> — {regsDesc} Non-zero values are highlighted.
              </li>
              <li>
                <strong>Memory</strong> — Hex dump of RAM. Use <code>Jump</code> to jump to an address. For memory &gt; 64 KB, chunks are shown; click <code>Open</code> on a chunk to view its contents. Use ⚙ Settings to change memory size (4 KB–1 MB).
              </li>
              <li>
                <strong>Trace</strong> — Shows pipeline events (FETCH, DECODE, ALU, MEM, REG_WRITE) for each step.
              </li>
              <li>
                <strong>Clock Panel</strong> — Shows cycles, run state, mode. Click the cycle wave button to open the Cycle Timing Graph (5-stage pipeline breakdown).
              </li>
            </ul>
          </section>

          {arch === "RV32I" && (
            <>
              <section>
                <h3>3. Architecture: RISC-V RV32I</h3>
                <p>
                  <strong>RISC-V RV32I</strong> is a 32-bit integer instruction set. The simulator models a 5-stage pipeline:
                  Fetch → Decode → Execute → Memory → Write-back.
                </p>
                <h4>Registers (x0–x31)</h4>
                <table className="help-table">
                  <thead>
                    <tr>
                      <th>Register</th>
                      <th>ABI Name</th>
                      <th>Description</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr><td>x0</td><td>zero</td><td>Always 0 (read-only)</td></tr>
                    <tr><td>x1</td><td>ra</td><td>Return address</td></tr>
                    <tr><td>x2</td><td>sp</td><td>Stack pointer</td></tr>
                    <tr><td>x3</td><td>gp</td><td>Global pointer</td></tr>
                    <tr><td>x4</td><td>tp</td><td>Thread pointer</td></tr>
                    <tr><td>x5–x7</td><td>t0–t2</td><td>Temporaries</td></tr>
                    <tr><td>x8–x9</td><td>s0–s1</td><td>Saved</td></tr>
                    <tr><td>x10–x17</td><td>a0–a7</td><td>Arguments / return values</td></tr>
                    <tr><td>x18–x27</td><td>s2–s11</td><td>Saved</td></tr>
                    <tr><td>x28–x31</td><td>t3–t6</td><td>Temporaries</td></tr>
                  </tbody>
                </table>
                <h4>Memory</h4>
                <p>Byte-addressable, little-endian. Word (32-bit) loads/stores are 4-byte aligned. Program starts at <code>_start</code> label (or address 0).</p>
              </section>

              <section>
                <h3>4. Assembly Syntax (RV32I)</h3>
                <ul>
                  <li><strong>Labels</strong> — <code>label:</code> defines a label. Use <code>_start:</code> as the program entry point.</li>
                  <li><strong>Comments</strong> — <code>#</code> or <code>//</code> to end of line.</li>
                  <li><strong>Registers</strong> — <code>x0</code>–<code>x31</code> or ABI names (<code>zero</code>, <code>ra</code>, etc.).</li>
                  <li><strong>Numbers</strong> — Decimal (<code>42</code>) or hex (<code>0x2a</code>).</li>
                </ul>
              </section>

              <section>
                <h3>5. Instruction Reference (RV32I subset)</h3>
                <table className="help-table help-instr">
                  <thead>
                    <tr>
                      <th>Instruction</th>
                      <th>Syntax</th>
                      <th>Description</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr><td><code>addi</code></td><td><code>addi rd, rs1, imm</code></td><td>rd = rs1 + sign-extended imm (12-bit)</td></tr>
                    <tr><td><code>add</code></td><td><code>add rd, rs1, rs2</code></td><td>rd = rs1 + rs2</td></tr>
                    <tr><td><code>sub</code></td><td><code>sub rd, rs1, rs2</code></td><td>rd = rs1 − rs2</td></tr>
                    <tr><td><code>lui</code></td><td><code>lui rd, imm</code></td><td>rd = imm &lt;&lt; 12 (load upper immediate)</td></tr>
                    <tr><td><code>lw</code></td><td><code>lw rd, offset(rs1)</code></td><td>rd = Mem[rs1 + offset] (32-bit load)</td></tr>
                    <tr><td><code>sw</code></td><td><code>sw rs2, offset(rs1)</code></td><td>Mem[rs1 + offset] = rs2 (32-bit store)</td></tr>
                    <tr><td><code>beq</code></td><td><code>beq rs1, rs2, label</code></td><td>Branch if rs1 == rs2</td></tr>
                    <tr><td><code>bne</code></td><td><code>bne rs1, rs2, label</code></td><td>Branch if rs1 != rs2</td></tr>
                    <tr><td><code>j</code></td><td><code>j label</code></td><td>Unconditional jump</td></tr>
                    <tr><td><code>jal</code></td><td><code>jal rd, label</code></td><td>rd = PC+4; PC = label</td></tr>
                    <tr><td><code>jalr</code></td><td><code>jalr rd, offset(rs1)</code></td><td>rd = PC+4; PC = (rs1 + offset) &amp; ~1</td></tr>
                    <tr><td><code>ecall</code></td><td><code>ecall</code></td><td>a7=10: exit, a7=11: print int (a0), a7=12: print char (a0)</td></tr>
                    <tr><td><code>li</code></td><td><code>li rd, imm</code></td><td>Load immediate</td></tr>
                    <tr><td><code>mv</code></td><td><code>mv rd, rs1</code></td><td>Move (addi rd, rs1, 0)</td></tr>
                    <tr><td><code>ret</code></td><td><code>ret</code></td><td>Return (jalr x0, 0(ra))</td></tr>
                    <tr><td><code>nop</code></td><td><code>nop</code></td><td>No operation</td></tr>
                  </tbody>
                </table>
              </section>

              <section>
                <h3>8. Official RISC-V Documentation</h3>
                <p>For the complete instruction set and architecture specification:</p>
                <ul className="help-links">
                  <li>
                    <button type="button" className="help-link-btn" onClick={() => handleOpenLink("https://docs.riscv.org/reference/isa/_attachments/riscv-unprivileged.pdf")}>
                      RISC-V Unprivileged ISA Specification (PDF)
                    </button>
                    {" "}— Full RV32I, RV64I, and other base extensions
                  </li>
                  <li>
                    <button type="button" className="help-link-btn" onClick={() => handleOpenLink("https://docs.riscv.org/reference/isa/unpriv/rv32.html")}>
                      RV32I Base Integer Instruction Set (web)
                    </button>
                  </li>
                </ul>
              </section>
            </>
          )}

          {arch === "LC3" && (
            <>
              <section>
                <h3>3. Architecture: LC-3</h3>
                <p>
                  <strong>LC-3 (Little Computer 3)</strong> is a 16-bit educational instruction set. 8 registers (R0–R7), 16-bit address space.
                  Entry point typically <code>.ORIG x3000</code>. 5-stage pipeline.
                </p>
                <h4>Registers (R0–R7)</h4>
                <p>R0–R7 are general-purpose. R7 is used as return address by JSR/JSRR. Condition codes (N, Z, P) are set by ADD, AND, NOT, LD, LDI, LDR, LEA.</p>
                <h4>Memory</h4>
                <p>Word-addressable (16-bit), little-endian. Program starts at <code>.ORIG</code> address (default x3000).</p>
              </section>

              <section>
                <h3>4. Assembly Syntax (LC-3)</h3>
                <ul>
                  <li><strong>Directive</strong> — <code>.ORIG x3000</code> sets program start address.</li>
                  <li><strong>Labels</strong> — <code>label:</code> defines a label. Use <code>_start:</code> as entry.</li>
                  <li><strong>Comments</strong> — <code>;</code> or <code>#</code> to end of line.</li>
                  <li><strong>Registers</strong> — <code>R0</code>–<code>R7</code>.</li>
                  <li><strong>Numbers</strong> — Decimal (<code>#42</code>) or hex (<code>x2a</code>).</li>
                </ul>
              </section>

              <section>
                <h3>5. Instruction Reference (LC-3)</h3>
                <table className="help-table help-instr">
                  <thead>
                    <tr>
                      <th>Instruction</th>
                      <th>Syntax</th>
                      <th>Description</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr><td><code>ADD</code></td><td><code>ADD DR, SR1, SR2</code> or <code>ADD DR, SR1, #imm5</code></td><td>DR = SR1 + SR2 or imm5</td></tr>
                    <tr><td><code>AND</code></td><td><code>AND DR, SR1, SR2</code> or <code>AND DR, SR1, #imm5</code></td><td>DR = SR1 &amp; SR2 or imm5</td></tr>
                    <tr><td><code>NOT</code></td><td><code>NOT DR, SR</code></td><td>DR = ~SR</td></tr>
                    <tr><td><code>BR</code></td><td><code>BRn / BRz / BRp / BRnzp label</code></td><td>Branch if condition codes match</td></tr>
                    <tr><td><code>JMP</code></td><td><code>JMP BaseR</code></td><td>PC = BaseR</td></tr>
                    <tr><td><code>RET</code></td><td><code>RET</code></td><td>JMP R7</td></tr>
                    <tr><td><code>JSR</code></td><td><code>JSR label</code></td><td>R7 = PC+2; PC = label</td></tr>
                    <tr><td><code>JSRR</code></td><td><code>JSRR BaseR</code></td><td>R7 = PC+2; PC = BaseR</td></tr>
                    <tr><td><code>LD</code></td><td><code>LD DR, label</code></td><td>DR = Mem[PC + offset9]</td></tr>
                    <tr><td><code>LDI</code></td><td><code>LDI DR, label</code></td><td>DR = Mem[Mem[PC + offset9]]</td></tr>
                    <tr><td><code>LDR</code></td><td><code>LDR DR, BaseR, offset6</code></td><td>DR = Mem[BaseR + offset6]</td></tr>
                    <tr><td><code>LEA</code></td><td><code>LEA DR, label</code></td><td>DR = PC + offset9</td></tr>
                    <tr><td><code>ST</code></td><td><code>ST SR, label</code></td><td>Mem[PC + offset9] = SR</td></tr>
                    <tr><td><code>STI</code></td><td><code>STI SR, label</code></td><td>Mem[Mem[PC + offset9]] = SR</td></tr>
                    <tr><td><code>STR</code></td><td><code>STR SR, BaseR, offset6</code></td><td>Mem[BaseR + offset6] = SR</td></tr>
                    <tr><td><code>TRAP</code></td><td><code>TRAP trapvect8</code></td><td>x20=OUT (R0), x25=HALT</td></tr>
                    <tr><td><code>HALT</code></td><td><code>HALT</code></td><td>TRAP x25 (stop execution)</td></tr>
                    <tr><td><code>NOP</code></td><td><code>NOP</code></td><td>No operation</td></tr>
                  </tbody>
                </table>
              </section>

              <section>
                <h3>8. LC-3 Documentation</h3>
                <ul className="help-links">
                  <li>
                    <button type="button" className="help-link-btn" onClick={() => handleOpenLink("https://en.wikipedia.org/wiki/Little_Computer_3")}>
                      LC-3 (Wikipedia)
                    </button>
                    {" "}— Overview of the instruction set
                  </li>
                </ul>
              </section>
            </>
          )}

          {arch === "MIPS" && (
            <>
              <section>
                <h3>3. Architecture: MIPS</h3>
                <p>
                  <strong>MIPS32</strong> is a 32-bit RISC instruction set. 32 registers ($0–$31), 32-bit address space.
                  $0 (zero) is always 0. $31 (ra) is return address. $2–$3 (v0–v1) for return values, $4–$7 (a0–a3) for args.
                  syscall uses $v0 for service number; $v0=10 exits, $v0=1 prints int, $v0=11 prints char.
                </p>
                <h4>Registers</h4>
                <p>$0 (zero), $2–$3 (v0–v1), $4–$7 (a0–a3), $8–$15 (t0–t7), $16–$23 (s0–s7), $29 (sp), $31 (ra).</p>
                <h4>Memory</h4>
                <p>Byte-addressable, little-endian. Word loads/stores 4-byte aligned. Program starts at <code>_start</code> or <code>main</code> (address 0).</p>
              </section>

              <section>
                <h3>4. Assembly Syntax (MIPS)</h3>
                <ul>
                  <li><strong>Labels</strong> — <code>label:</code> defines a label. Use <code>_start:</code> or <code>main:</code> as entry.</li>
                  <li><strong>Comments</strong> — <code>#</code> to end of line.</li>
                  <li><strong>Registers</strong> — <code>$0</code>–<code>$31</code> or names (<code>$zero</code>, <code>$v0</code>, <code>$a0</code>, <code>$t0</code>, etc.).</li>
                  <li><strong>Numbers</strong> — Decimal (<code>42</code>) or hex (<code>0x2a</code>).</li>
                </ul>
              </section>

              <section>
                <h3>5. Instruction Reference (MIPS subset)</h3>
                <table className="help-table help-instr">
                  <thead>
                    <tr>
                      <th>Instruction</th>
                      <th>Syntax</th>
                      <th>Description</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr><td><code>add</code></td><td><code>add rd, rs, rt</code></td><td>rd = rs + rt</td></tr>
                    <tr><td><code>sub</code></td><td><code>sub rd, rs, rt</code></td><td>rd = rs − rt</td></tr>
                    <tr><td><code>and</code></td><td><code>and rd, rs, rt</code></td><td>rd = rs &amp; rt</td></tr>
                    <tr><td><code>or</code></td><td><code>or rd, rs, rt</code></td><td>rd = rs | rt</td></tr>
                    <tr><td><code>addi</code></td><td><code>addi rt, rs, imm</code></td><td>rt = rs + sign-extended imm</td></tr>
                    <tr><td><code>lw</code></td><td><code>lw rt, offset(rs)</code></td><td>rt = Mem[rs + offset]</td></tr>
                    <tr><td><code>sw</code></td><td><code>sw rt, offset(rs)</code></td><td>Mem[rs + offset] = rt</td></tr>
                    <tr><td><code>beq</code></td><td><code>beq rs, rt, label</code></td><td>Branch if rs == rt</td></tr>
                    <tr><td><code>bne</code></td><td><code>bne rs, rt, label</code></td><td>Branch if rs != rt</td></tr>
                    <tr><td><code>j</code></td><td><code>j label</code></td><td>Unconditional jump</td></tr>
                    <tr><td><code>jal</code></td><td><code>jal label</code></td><td>$ra = PC+4; PC = label</td></tr>
                    <tr><td><code>jr</code></td><td><code>jr rs</code></td><td>PC = rs</td></tr>
                    <tr><td><code>li</code></td><td><code>li rt, imm</code></td><td>Load immediate (addiu rt, $0, imm)</td></tr>
                    <tr><td><code>syscall</code></td><td><code>syscall</code></td><td>$v0=10: exit, $v0=1: print int ($a0), $v0=11: print char ($a0)</td></tr>
                    <tr><td><code>nop</code></td><td><code>nop</code></td><td>No operation</td></tr>
                  </tbody>
                </table>
              </section>

              <section>
                <h3>8. MIPS Documentation</h3>
                <ul className="help-links">
                  <li>
                    <button type="button" className="help-link-btn" onClick={() => handleOpenLink("https://en.wikibooks.org/wiki/MIPS_Assembly")}>
                      MIPS Assembly (Wikibooks)
                    </button>
                    {" "}— Instruction reference
                  </li>
                </ul>
              </section>
            </>
          )}

          <section>
            <h3>6. File Format (.asim)</h3>
            <p>
              Save and load projects as <code>.asim</code> files. The format stores source code, architecture, memory size, breakpoints, and settings. Compatible across RV32I, LC-3, and MIPS.
            </p>
          </section>

          <section>
            <h3>7. Sample Program ({arch})</h3>
            <pre className="help-code">{sampleCode}</pre>
          </section>
        </div>
      </div>
    </div>
  );
}
