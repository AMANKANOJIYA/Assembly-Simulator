# Screenshots guide

This folder holds optional screenshots referenced from the [main README](../README.md).

Add PNGs here and link them from `README.md` under **Screenshots** (see that section for an example).

## Suggested screenshot files

### 1. `main_window.png`
**Full application window** showing:
- Left panel: Monaco code editor with assembly code (preferably LC-3 or RISC-V)
- Center-top: Architecture diagram with pipeline stages visible
- Center-bottom: Registers panel showing register values
- Right-top: Memory hex dump viewer
- Right-bottom: Trace panel with execution events

**Recommended**: Take with a simple program running (e.g., LC-3 echo program)

### 2. `architecture_diagram.png`
**Close-up of architecture diagram** showing:
- PC block highlighted
- Active pipeline stage (e.g., Fetch highlighted in yellow)
- Data flow arrows between stages
- Register file and ALU blocks visible

**Recommended**: Capture during step execution when a stage is active

### 3. `step_execution.png`
**Application during step-by-step execution** showing:
- Code editor with current instruction highlighted (yellow/blue highlight)
- Registers panel with updated values (e.g., R1 = 10, R2 = 20)
- Trace panel showing recent events (FETCH, DECODE, ALU, REG_WRITE)
- Pipeline diagram showing active stage

**Recommended**: Take while stepping through a simple ADD instruction

### 4. `breakpoints.png`
**Breakpoint debugging** showing:
- Code editor with red breakpoint markers in the gutter (left of line numbers)
- Execution paused at breakpoint
- Toast notification visible: "Breakpoint hit"
- All panels showing state at breakpoint

**Recommended**: Set breakpoint on a branch instruction, run, show pause

### 5. `io_input.png`
**Runtime Console with I/O interaction** showing:
- "Trap/Interrupt Input" section visible
- Input field for entering character/int/string
- "Send to Program" button
- "Program Output" section showing previously printed text
- Quick input buttons (char/int) if visible

**Recommended**: Use LC-3 TRAP x22 (IN) or RISC-V ecall 5 (read int)

### 6. `multi_arch.png`
**Architecture selector** showing:
- Architecture dropdown open with options:
  - RV32I (selected)
  - LC-3
  - MIPS
- Different register names visible based on selection
- Code editor showing architecture-specific syntax

**Recommended**: Show dropdown open with all three options visible

## Screenshot Tips

1. **Resolution**: Use at least 1920x1080 or higher
2. **Format**: PNG format preferred for clarity
3. **Window**: Use full application window, not cropped
4. **Content**: Use meaningful sample programs (not just "nop" or empty code)
5. **Highlighting**: Ensure active elements are clearly visible (highlights, selected text, etc.)
6. **Consistency**: Use similar code samples across screenshots for coherence

## How to Take Screenshots

### macOS
- **Full window**: `Cmd + Shift + 4`, then press `Space`, click window
- **Selected area**: `Cmd + Shift + 4`, drag to select
- **Save location**: Desktop (then move to `screenshots/` folder)

### Linux
- Use `gnome-screenshot` or `scrot`
- Or use built-in screenshot tool

### Windows
- `Win + Shift + S` for Snipping Tool
- Or `PrtScn` for full screen

## After Taking Screenshots

1. Rename files to match the names above
2. Place in `screenshots/` directory
3. Update main README.md to reference the actual images (remove `<!-- Add screenshot: ... -->` comments and add `![Description](screenshots/filename.png)`)

Example:
```markdown
### Main Interface
![Main Interface](screenshots/main_window.png)
```
