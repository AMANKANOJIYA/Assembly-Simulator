# Assembly Simulator + Architecture Visualizer

<div align="center">

**A cross-platform desktop application for learning and teaching computer architecture through interactive assembly simulation**

[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-2C2D72?logo=tauri)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-1.0+-orange?logo=rust)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-18+-61dafb?logo=react)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0+-3178c6?logo=typescript)](https://www.typescriptlang.org/)

</div>

---

## 📖 Overview

**Assembly Simulator** is an educational desktop application designed to help students and educators understand computer architecture by simulating assembly language execution. The application provides a visual, step-by-step view of how instructions flow through a CPU pipeline, how registers change, and how memory is accessed.

### Use Cases

- **Education**: Teach computer architecture, assembly programming, and CPU internals
- **Learning**: Understand instruction execution, pipeline stages, and register/memory operations
- **Debugging**: Step through assembly code to identify bugs and understand program flow
- **Research**: Experiment with different architectures (RISC-V, LC-3, MIPS) side-by-side

### Key Features

✅ **Multi-Architecture Support**: RISC-V RV32I, LC-3, and MIPS  
✅ **Visual Pipeline**: See instructions flow through Fetch, Decode, Execute, Memory, Write-back stages  
✅ **Interactive Debugging**: Step forward/backward, breakpoints, variable-speed execution  
✅ **I/O Simulation**: Handle input/output via syscalls/traps (read char/int/string, print)  
✅ **Memory Visualization**: Hex dump with jump-to-address and configurable size  
✅ **Register Viewer**: Real-time register values with architecture-specific names  
✅ **Undo Support**: Step backward through execution (including I/O output)  
✅ **Error Highlighting**: Monaco editor with inline assembler error markers  

---

## 🖼️ Screenshots

### Main Interface
<!-- Add screenshot: screenshots/main_window.png -->
**Main Interface** - The application window showing the code editor, architecture diagram, registers panel, memory viewer, and trace panel.

*Screenshot description: Full application window with:*
- *Left: Monaco code editor with LC-3 assembly code*
- *Center-top: Architecture diagram showing pipeline stages (PC → Fetch → Decode → ALU → Memory → RegFile)*
- *Center-bottom: Registers panel showing R0-R7 and PSR*
- *Right-top: Memory hex dump viewer*
- *Right-bottom: Trace panel showing execution events*

### Architecture Diagram
<!-- Add screenshot: screenshots/architecture_diagram.png -->
**Architecture Diagram** - Visual representation of the CPU pipeline with active stage highlighting.

*Screenshot description: Close-up of the architecture diagram showing:*
- *PC block highlighted in blue*
- *Fetch stage active (yellow highlight)*
- *Data flow arrows between stages*
- *Register file and ALU blocks*

### Step-by-Step Execution
<!-- Add screenshot: screenshots/step_execution.png -->
**Step-by-Step Execution** - Stepping through code with register and memory updates visible.

*Screenshot description: Application during step execution showing:*
- *Code editor with current instruction highlighted*
- *Registers panel showing updated values (R1 = 10, R2 = 20)*
- *Trace panel showing "FETCH", "DECODE", "ALU", "REG_WRITE" events*
- *Pipeline stages showing which stage is active*

### Breakpoint Debugging
<!-- Add screenshot: screenshots/breakpoints.png -->
**Breakpoint Debugging** - Setting breakpoints and pausing execution.

*Screenshot description: Code editor with:*
- *Red breakpoint markers in the gutter*
- *Execution paused at breakpoint*
- *Toast notification: "Breakpoint hit"*
- *All panels showing state at breakpoint*

### I/O Interaction
<!-- Add screenshot: screenshots/io_input.png -->
**I/O Interaction** - Runtime console showing input request and output.

*Screenshot description: Runtime Console panel showing:*
- *"Trap/Interrupt Input" section*
- *Input field for entering character*
- *"Send to Program" button*
- *Program Output section showing previously printed text*

### Multi-Architecture Comparison
<!-- Add screenshot: screenshots/multi_arch.png -->
**Multi-Architecture Comparison** - Switching between RISC-V, LC-3, and MIPS.

*Screenshot description: Architecture selector dropdown showing:*
- *RV32I selected*
- *LC-3 option*
- *MIPS option*
- *Different register names visible (x0-x31 vs R0-R7 vs $zero-$ra)*

---

## 🚀 Quick Start

### Prerequisites

- **Node.js** 18+ and npm
- **Rust** (install via [rustup](https://rustup.rs/))
- **macOS** (primary target; Linux/Windows may work but not tested)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd simulator

# Install dependencies
npm install

# Run in development mode
npm run tauri:dev
```

### Building for Production

```bash
npm run tauri:build
```

The built application will be in `src-tauri/target/release/bundle/macos/` (macOS) or equivalent for your platform.

---

## 📚 Supported Architectures

### RISC-V RV32I

**ALU Instructions**: `addi`, `add`, `sub`, `slt`, `sltu`, `slti`, `sltiu`, `xor`, `xori`, `or`, `ori`, `and`, `andi`, `sll`, `srl`, `sra`, `slli`, `srli`, `srai`, `lui`

**Memory Instructions**: `lb`, `lh`, `lw`, `lbu`, `lhu`, `sb`, `sh`, `sw`

**Branch Instructions**: `beq`, `bne`, `blt`, `bge`, `bltu`, `bgeu`

**Jump Instructions**: `jal`, `jalr`, `j`, `ret`, `mv`, `li`, `nop`

**System Calls** (via `ecall`, register `a7`):
- `4` = Print string (a0 = address)
- `5` = Read integer → a0
- `8` = Read string → buffer at a0, max length a1
- `10` = Exit
- `11` = Print integer (a0)
- `12` = Print character (a0)
- `13` = Read character → a0

**Example**:
```asm
_start:
  addi a0, x0, 42
  addi a7, x0, 11    # print int
  ecall
  addi a7, x0, 10    # exit
  ecall
```

### LC-3

**Instructions**: `ADD`, `AND`, `NOT`, `BR`, `JMP`, `JSR`, `JSRR`, `LD`, `LDI`, `LDR`, `LEA`, `ST`, `STI`, `STR`, `TRAP`, `HALT`, `NOP`

**Directives**: `.ORIG`, `.FILL`, `.BLKW`, `.END`

**TRAP Codes**:
- `x20` = OUT (print char in R0)
- `x21` = PUTS (print string at R0)
- `x22` = IN (read char → R0, with echo)
- `x23` = GETC (read char → R0, no echo)
- `x25` = HALT

**Example**:
```asm
.ORIG x3000
_start:
  TRAP x22          ; IN: read char → R0
  TRAP x20          ; OUT: print char in R0
  HALT
.END
```

### MIPS

**Instructions**: `add`, `sub`, `and`, `or`, `addi`, `lw`, `sw`, `beq`, `bne`, `j`, `jal`, `jr`, `syscall`, `li`, `nop`

**System Calls** (via `syscall`, register `$v0`):
- `1` = Print integer ($a0)
- `4` = Print string ($a0 = address)
- `5` = Read integer → $v0
- `8` = Read string → buffer at $a0, max length $a1
- `10` = Exit
- `11` = Print character ($a0)
- `12` = Read character → $v0

**Example**:
```asm
_start:
  addi $a0, $zero, 72
  addi $v0, $zero, 11    # print char
  syscall
  addi $v0, $zero, 10    # exit
  syscall
```

---

## 🎮 Usage Guide

### Writing Code

1. **Select Architecture**: Use the dropdown in the top toolbar to choose RV32I, LC-3, or MIPS
2. **Write Assembly**: Type your assembly code in the Monaco editor
3. **Use Labels**: Define labels with a colon (e.g., `_start:`, `loop:`)
4. **Comments**: 
   - RISC-V/MIPS: Use `#` for comments
   - LC-3: Use `;` for comments

### Running Programs

1. **Assemble**: Click "Assemble" to check for errors (or it happens automatically on Run)
2. **Run**: Click "Run" to execute at full speed
3. **Pause**: Click "Pause" to stop execution
4. **Step Forward**: Execute one instruction at a time
5. **Step Back**: Undo the last instruction (including I/O output)
6. **Reset**: Restart the program from the beginning

### Debugging

1. **Breakpoints**: Click in the gutter (left of line numbers) to set/remove breakpoints
2. **Breakpoint Hit**: Execution pauses automatically when PC reaches a breakpoint
3. **Inspect State**: View registers, memory, and trace events while paused
4. **Step Through**: Use Step Forward/Back to examine execution in detail

### I/O Interaction

1. **Input Request**: When a program calls a read syscall/trap, a "Trap/Interrupt Input" panel appears
2. **Enter Input**: Type your input (char, int, or string) and click "Send to Program"
3. **Output**: Printed text appears in the "Program Output" section
4. **Auto-Continue**: After sending input, execution continues automatically

### Memory Management

1. **View Memory**: Scroll through the memory hex dump
2. **Jump to Address**: Type an address (e.g., `0x3000`) and click "Jump"
3. **Change Size**: Click the settings icon (⚙) to change memory size (4KB–1MB)
4. **Memory Chunks**: For large memory (>64KB), view by chunks

---

## 🏗️ Architecture

### Project Structure

```
simulator/
├── src/                          # React frontend (TypeScript)
│   ├── components/
│   │   ├── Editor.tsx           # Monaco code editor
│   │   ├── Controls.tsx         # Run/Pause/Step buttons
│   │   ├── DiagramPanel.tsx     # Architecture diagram
│   │   ├── RegistersPanel.tsx   # Register viewer
│   │   ├── MemoryPanel.tsx      # Memory hex dump
│   │   ├── TracePanel.tsx       # Execution trace
│   │   └── RuntimeConsole.tsx   # I/O input/output
│   ├── store.ts                 # Zustand state management
│   ├── samples.ts               # Sample programs
│   └── types.ts                 # TypeScript types
│
├── src-tauri/                   # Rust backend
│   └── src/
│       ├── plugin/              # Architecture plugins
│       │   ├── mod.rs           # ArchitecturePlugin trait
│       │   ├── adapter.rs       # Architecture config/registry
│       │   ├── rv32i.rs         # RISC-V implementation
│       │   ├── lc3.rs           # LC-3 implementation
│       │   └── mips.rs          # MIPS implementation
│       ├── simulator.rs         # CPU state, undo stack, breakpoints
│       ├── memory.rs            # Memory abstraction
│       ├── commands.rs          # Tauri IPC commands
│       └── lib.rs               # Tauri app entry
│
└── README.md
```

### Architecture Plugin System

The application uses a plugin-based architecture that makes it easy to add new instruction set architectures:

1. **Implement `ArchitecturePlugin` trait**:
   - `assemble()`: Parse assembly source → binary
   - `step()`: Execute one instruction
   - `reset()`: Initialize CPU state
   - `ui_schema()`: Define diagram layout
   - `register_schema()`: Define register names

2. **Register in adapter**: Add config in `plugin/adapter.rs`

3. **Add to simulator**: Register plugin in `simulator.rs::get_plugin()`

4. **Add samples**: Create sample programs in `src/samples.ts`

### Data Flow

```
User Input (Editor)
    ↓
Frontend (React/TypeScript)
    ↓ [Tauri IPC]
Backend (Rust)
    ↓
Architecture Plugin (RV32I/LC-3/MIPS)
    ↓
Simulator (State + Memory)
    ↓
Step Result (Registers, Memory, Events)
    ↓ [Tauri IPC]
Frontend (Update UI)
```

---

## 📝 Sample Programs

### RISC-V: Hello World
```asm
_start:
  lui  a0, 0x10000      # Load address
  addi a0, a0, 72       # 'H'
  addi a7, x0, 12       # print char
  ecall
  addi a0, x0, 105      # 'i'
  ecall
  addi a7, x0, 10       # exit
  ecall
```

### LC-3: Echo Input
```asm
.ORIG x3000
_start:
  TRAP x22              ; IN: read char → R0
  TRAP x20              ; OUT: print char in R0
  ADD  R0, R0, #0
  BRz  done             ; if 0, exit
  LD   R0, newline
  TRAP x20
  BRnzp _start
done:
  HALT
newline: .FILL x000A
.END
```

### MIPS: Add Two Numbers
```asm
_start:
  addi $t0, $zero, 10
  addi $t1, $zero, 20
  add  $t2, $t0, $t1
  addi $v0, $zero, 1     # print int
  add  $a0, $zero, $t2
  syscall
  addi $v0, $zero, 10    # exit
  syscall
```

---

## 🔧 Development

### Adding a New Architecture

1. **Create plugin file**: `src-tauri/src/plugin/<arch>.rs`
   ```rust
   pub struct <Arch>Plugin;
   impl ArchitecturePlugin for <Arch>Plugin { ... }
   ```

2. **Register in `mod.rs`**: Add module and export

3. **Add config**: Update `adapter.rs::arch_config()`

4. **Register plugin**: Add to `simulator.rs::get_plugin()`

5. **Add samples**: Create samples in `src/samples.ts`

6. **Update UI**: Add architecture option in file menu

### Building

```bash
# Development
npm run tauri:dev

# Production build
npm run tauri:build

# Check Rust code
cd src-tauri && cargo check

# Format Rust code
cd src-tauri && cargo fmt
```

---

## 🐛 Known Limitations

- **Step Back I/O**: I/O output is now properly undone ✅ (fixed)
- **Memory Size**: User-configured memory size is now respected ✅ (fixed)
- **Breakpoints**: Backend breakpoint support is now implemented ✅ (fixed)
- **Platform Support**: Primarily tested on macOS; Linux/Windows may have issues

---

## 📄 License

MIT License - see LICENSE file for details

---

## 🙏 Acknowledgments

- Built with [Tauri](https://tauri.app/) for cross-platform desktop apps
- Uses [Monaco Editor](https://microsoft.github.io/monaco-editor/) for code editing
- Architecture diagrams inspired by Patterson & Hennessy's "Computer Organization and Design"

---

## 📧 Contributing

Contributions welcome! Please open an issue or submit a pull request.

---

<div align="center">

**Made with ❤️ for computer architecture education**

</div>
