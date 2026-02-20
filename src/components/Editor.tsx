import { useRef, useEffect, useMemo } from "react";
import MonacoEditor from "@monaco-editor/react";
import type { editor } from "monaco-editor";
import type * as Monaco from "monaco-editor";
import { useStore } from "../store";

interface EditorPanelProps {
  onAssemble?: () => void;
}

function pcToLine(sourceMap: { pc: number; line: number }[] | undefined, pc: number): number | null {
  if (!sourceMap) return null;
  const entry = sourceMap.find((e) => e.pc === pc);
  return entry ? entry.line : null;
}

function lineToPc(sourceMap: { pc: number; line: number }[] | undefined, line: number): number | null {
  if (!sourceMap) return null;
  const entry = sourceMap.find((e) => e.line === line);
  return entry ? entry.pc : null;
}

export function EditorPanel(_props: EditorPanelProps) {
  const source = useStore((s) => s.source);
  const setSource = useStore((s) => s.setSource);
  const errors = useStore((s) => s.errors);
  const snapshot = useStore((s) => s.snapshot);
  const breakpoints = useStore((s) => s.breakpoints);
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const decorationsRef = useRef<string[]>([]);

  const sourceMap = snapshot?.source_map ?? [];
  const currentPc = snapshot?.state.pc ?? null;
  const currentLine = useMemo(
    () => (currentPc != null ? pcToLine(sourceMap, currentPc) : null),
    [currentPc, sourceMap]
  );

  useEffect(() => {
    const ed = editorRef.current;
    const monaco = monacoRef.current;
    if (!ed || !monaco) return;
    const model = ed.getModel();
    if (!model) return;
    const markers: editor.IMarkerData[] = errors.map((e) => ({
      severity: monaco.MarkerSeverity.Error,
      message: e.message,
      startLineNumber: e.line,
      startColumn: e.column,
      endLineNumber: e.line,
      endColumn: Math.max(e.column + 1, 1),
    }));
    monaco.editor.setModelMarkers(model, "assembler", markers);
    return () => {
      monaco.editor.setModelMarkers(model, "assembler", []);
    };
  }, [errors]);

  useEffect(() => {
    const ed = editorRef.current;
    const monaco = monacoRef.current;
    if (!ed || !monaco) return;
    const model = ed.getModel();
    if (!model) return;

    const breakpointLines = new Set(
      breakpoints
        .map((pc) => pcToLine(sourceMap, pc))
        .filter((l): l is number => l != null)
    );

    const newDecos: editor.IModelDeltaDecoration[] = [];

    if (currentLine != null) {
      newDecos.push({
        range: { startLineNumber: currentLine, startColumn: 1, endLineNumber: currentLine, endColumn: 1 },
        options: {
          isWholeLine: true,
          className: "editor-pc-line",
          stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges,
        },
      });
    }

    for (const line of breakpointLines) {
      newDecos.push({
        range: { startLineNumber: line, startColumn: 1, endLineNumber: line, endColumn: 1 },
        options: {
          glyphMarginClassName: "editor-breakpoint",
          stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges,
        },
      });
    }

    decorationsRef.current = ed.deltaDecorations(decorationsRef.current, newDecos);
  }, [currentLine, breakpoints, sourceMap]);

  useEffect(() => {
    const ed = editorRef.current;
    if (!ed || currentLine == null) return;
    ed.revealLineInCenter(currentLine, 1);
  }, [currentLine]);

  return (
    <div className="panel editor-panel">
      <div className="panel-header">
        <h3>Code</h3>
      </div>
      <div className="editor-container">
        <MonacoEditor
          height="100%"
          defaultLanguage="asm"
          value={source}
          onChange={(v) => setSource(v ?? "")}
          onMount={(editor, monaco) => {
            editorRef.current = editor;
            monacoRef.current = monaco;
            editor.onMouseDown((e) => {
              const target = e.target;
              const GUTTER_GLYPH = 6; // MouseTargetType.GUTTER_GLYPH_MARGIN
              if (target.type === GUTTER_GLYPH && target.position) {
                const line = target.position.lineNumber;
                const sm = useStore.getState().snapshot?.source_map ?? [];
                const pc = lineToPc(sm, line);
                if (pc != null) useStore.getState().toggleBreakpoint(pc);
              }
            });
          }}
          options={{
            minimap: { enabled: false },
            fontSize: 13,
            fontFamily: "JetBrains Mono, Fira Code, monospace",
            lineNumbers: "on",
            glyphMargin: true,
            scrollBeyondLastLine: false,
            wordWrap: "on",
          }}
        />
      </div>
    </div>
  );
}
