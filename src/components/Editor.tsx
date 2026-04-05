import { useRef, useEffect, useMemo } from "react";
import MonacoEditor from "@monaco-editor/react";
import type { editor } from "monaco-editor";
import type * as Monaco from "monaco-editor";
import { useStore } from "../store";
import { MONO_FONT_STACKS } from "../appearance";

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

export function EditorPanel() {
  const activeEditorTabId = useStore((s) => s.activeEditorTabId);
  const source = useStore((s) => s.tabBuffers[s.activeEditorTabId] ?? "");
  const editorTabs = useStore((s) => s.editorTabs);
  const setSource = useStore((s) => s.setSource);
  const addEditorTab = useStore((s) => s.addEditorTab);
  const closeEditorTab = useStore((s) => s.closeEditorTab);
  const setActiveEditorTab = useStore((s) => s.setActiveEditorTab);

  const errors = useStore((s) => s.errors);
  const snapshot = useStore((s) => s.snapshot);
  const breakpoints = useStore((s) => s.breakpoints);
  const themeMode = useStore((s) => s.themeMode);
  const monoFontFamily = useStore((s) => s.monoFontFamily);
  const editorFontSize = useStore((s) => s.editorFontSize);
  const jumpToLineRequest = useStore((s) => s.jumpToLineRequest);
  const setJumpToLineRequest = useStore((s) => s.setJumpToLineRequest);
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const decorationsRef = useRef<string[]>([]);

  const sourceMap = snapshot?.source_map ?? [];
  const currentPc = snapshot?.state.pc ?? null;
  const currentLine = useMemo(
    () => (currentPc != null ? pcToLine(sourceMap, currentPc) : null),
    [currentPc, sourceMap]
  );

  const monoStack = useMemo(() => MONO_FONT_STACKS[monoFontFamily], [monoFontFamily]);

  useEffect(() => {
    const ed = editorRef.current;
    if (!ed) return;
    ed.updateOptions({
      fontSize: editorFontSize,
      fontFamily: monoStack,
    });
  }, [editorFontSize, monoStack]);

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

  // Jump to a specific line when requested (e.g. from error/breakpoint panels)
  useEffect(() => {
    if (jumpToLineRequest == null) return;
    const ed = editorRef.current;
    if (ed) {
      ed.revealLineInCenter(jumpToLineRequest, 1);
      ed.setPosition({ lineNumber: jumpToLineRequest, column: 1 });
      ed.focus();
    }
    setJumpToLineRequest(null);
  }, [jumpToLineRequest, setJumpToLineRequest]);

  return (
    <div className="panel editor-panel" data-tour="editor">
      <div className="editor-tab-strip">
        <div className="editor-tab-list" role="tablist">
          {editorTabs.map((tab) => (
            <div
              key={tab.id}
              className={`editor-tab${tab.id === activeEditorTabId ? " is-active" : ""}`}
              role="tab"
              aria-selected={tab.id === activeEditorTabId}
            >
              <button
                type="button"
                className="editor-tab-label"
                onClick={() => setActiveEditorTab(tab.id)}
                title={tab.filePath ?? tab.title}
              >
                {tab.title}
              </button>
              {editorTabs.length > 1 && (
                <button
                  type="button"
                  className="editor-tab-close"
                  aria-label={`Close ${tab.title}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    closeEditorTab(tab.id);
                  }}
                >
                  ×
                </button>
              )}
            </div>
          ))}
          <button type="button" className="editor-tab-add" onClick={() => addEditorTab()} title="New editor tab">
            +
          </button>
        </div>
      </div>
      <div className="editor-container">
        <MonacoEditor
          key={activeEditorTabId}
          height="100%"
          theme={themeMode === "dark" ? "vs-dark" : "vs"}
          defaultLanguage="asm"
          value={source}
          onChange={(v) => setSource(v ?? "")}
          onMount={(editor, monaco) => {
            editorRef.current = editor;
            monacoRef.current = monaco;
            editor.onMouseDown((e) => {
              const target = e.target;
              const GUTTER_GLYPH = 6;
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
            fontSize: editorFontSize,
            fontFamily: monoStack,
            lineNumbers: "on",
            glyphMargin: true,
            scrollBeyondLastLine: false,
            wordWrap: "on",
            smoothScrolling: true,
            cursorBlinking: "smooth",
            padding: { top: 8, bottom: 12 },
            find: {
              addExtraSpaceOnTop: false,
              autoFindInSelection: "never",
              seedSearchStringFromSelection: "always",
            },
          }}
        />
      </div>
    </div>
  );
}
