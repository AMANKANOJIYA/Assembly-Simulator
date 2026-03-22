import { useEffect } from "react";
import { useStore } from "../store";
import { UI_FONT_STACKS, MONO_FONT_STACKS } from "../appearance";

/** Applies font stacks, editor size, density, and motion prefs to <html> (CSS variables + data attributes). */
export function AppearanceSync() {
  const uiFontFamily = useStore((s) => s.uiFontFamily);
  const monoFontFamily = useStore((s) => s.monoFontFamily);
  const editorFontSize = useStore((s) => s.editorFontSize);
  const uiDensity = useStore((s) => s.uiDensity);
  const reducedMotion = useStore((s) => s.reducedMotion);

  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--font-ui", UI_FONT_STACKS[uiFontFamily]);
    root.style.setProperty("--font-mono", MONO_FONT_STACKS[monoFontFamily]);
    root.style.setProperty("--editor-font-size", `${editorFontSize}px`);
    root.dataset.uiDensity = uiDensity;
    root.dataset.reducedMotion = reducedMotion ? "true" : "false";
  }, [uiFontFamily, monoFontFamily, editorFontSize, uiDensity, reducedMotion]);

  return null;
}
