/** User-selectable UI / code font stacks (loaded via index.html Google Fonts subset). */

export type UiFontId = "ibm-plex" | "inter" | "system" | "segoe";
export type MonoFontId = "jetbrains" | "fira" | "source-code" | "system-mono";
export type UiDensity = "comfortable" | "compact";

export const UI_FONT_LABELS: Record<UiFontId, string> = {
  "ibm-plex": "IBM Plex Sans",
  inter: "Inter",
  system: "System UI",
  segoe: "Segoe UI stack",
};

export const MONO_FONT_LABELS: Record<MonoFontId, string> = {
  jetbrains: "JetBrains Mono",
  fira: "Fira Code",
  "source-code": "Source Code Pro",
  "system-mono": "System monospace",
};

export const UI_FONT_STACKS: Record<UiFontId, string> = {
  "ibm-plex": '"IBM Plex Sans", system-ui, -apple-system, sans-serif',
  inter: '"Inter", system-ui, -apple-system, sans-serif',
  system: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
  segoe: '"Segoe UI", Roboto, system-ui, sans-serif',
};

export const MONO_FONT_STACKS: Record<MonoFontId, string> = {
  jetbrains: '"JetBrains Mono", "Fira Code", ui-monospace, monospace',
  fira: '"Fira Code", "JetBrains Mono", ui-monospace, monospace',
  "source-code": '"Source Code Pro", "JetBrains Mono", Consolas, monospace',
  "system-mono": 'ui-monospace, SFMono-Regular, "Cascadia Code", Consolas, monospace',
};

export const EDITOR_FONT_SIZE = { min: 11, max: 20, default: 13 } as const;
