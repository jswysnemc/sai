import { beforeEach, describe, expect, it } from "vitest";
import {
  adjustEditorFontSize,
  applyFontPreferences,
  editorFontWheelDelta,
  loadFontPreferences,
  monacoFontOptions,
  normalizeFontPreferences,
  saveFontPreferences
} from "./font-preferences";

describe("font preferences", () => {
  const style = new Map<string, string>();
  const stored = new Map<string, string>();

  beforeEach(() => {
    style.clear();
    stored.clear();
    Object.assign(globalThis, {
      document: {
        documentElement: {
          style: {
            setProperty: (name: string, value: string) => style.set(name, value),
            getPropertyValue: (name: string) => style.get(name) ?? "",
            removeProperty: (name: string) => { style.delete(name); }
          }
        }
      },
      localStorage: {
        getItem: (key: string) => stored.get(key) ?? null,
        setItem: (key: string, value: string) => stored.set(key, value),
        removeItem: (key: string) => { stored.delete(key); }
      }
    });
    saveFontPreferences({ uiFont: "", editorFont: "", cjkFont: "", editorFontSize: 13 });
    stored.clear();
  });

  it("keeps the built-in fonts when storage is empty", () => {
    expect(loadFontPreferences()).toEqual({ uiFont: "", editorFont: "", cjkFont: "", editorFontSize: 13 });
  });

  it("drops unsafe font names and clamps editor size", () => {
    expect(normalizeFontPreferences({
      uiFont: "YaHei; color: red",
      editorFont: "JetBrains Mono",
      editorFontSize: 80
    })).toEqual({ uiFont: "YaHei color red", editorFont: "JetBrains Mono", cjkFont: "", editorFontSize: 32 });
    expect(normalizeFontPreferences({ editorFontSize: 4 }).editorFontSize).toBe(8);
  });

  it("writes interface and editor fonts onto the document root", () => {
    applyFontPreferences({ uiFont: "Source Han Sans SC", editorFont: "Sarasa Mono SC", cjkFont: "霞鹜文楷等宽", editorFontSize: 16 });
    expect(style.get("--font-ui")).toBe('"Source Han Sans SC", var(--font-ui-builtin)');
    expect(style.get("--font-editor")).toBe('"Sarasa Mono SC", var(--font-code)');
    expect(style.get("--font-cjk")).toBe('"霞鹜文楷等宽", var(--font-cjk-mono), var(--font-cjk-proportional)');
    expect(style.get("--editor-font-size")).toBe("16px");
    expect(style.get("--editor-line-height")).toBe("26px");

    applyFontPreferences({ uiFont: "", editorFont: "", cjkFont: "", editorFontSize: 13 });
    expect(style.has("--font-ui")).toBe(false);
    expect(style.has("--font-editor")).toBe(false);
    expect(style.has("--font-cjk")).toBe(false);
  });

  it("changes only the editor size from ctrl wheel and ignores other modifiers", () => {
    saveFontPreferences({ uiFont: "Inter", editorFont: "", cjkFont: "", editorFontSize: 13 });
    expect(editorFontWheelDelta({ ctrlKey: false, metaKey: false, deltaY: -10 })).toBe(0);
    expect(editorFontWheelDelta({ ctrlKey: true, metaKey: true, deltaY: -10 })).toBe(0);
    expect(editorFontWheelDelta({ ctrlKey: true, metaKey: false, deltaY: -10 })).toBe(1);

    adjustEditorFontSize(1);
    expect(loadFontPreferences().editorFontSize).toBe(14);
    expect(loadFontPreferences().uiFont).toBe("Inter");
    adjustEditorFontSize(-1);
  });

  it("puts a custom editor family ahead of the monospace fallback", () => {
    const custom = monacoFontOptions({ uiFont: "", editorFont: "JetBrains Mono", cjkFont: "Sarasa Mono SC", editorFontSize: 18 }).fontFamily;
    expect(custom).toContain('"JetBrains Mono", "Fira Code"');
    expect(custom.indexOf('"Sarasa Mono SC"')).toBeGreaterThan(custom.indexOf('"Fira Code"'));
    const fallback = monacoFontOptions({ uiFont: "", editorFont: "", cjkFont: "", editorFontSize: 18 }).fontFamily;
    expect(fallback.indexOf('"Sarasa Mono SC"')).toBeLessThan(fallback.indexOf('"Noto Sans SC"'));
    expect(monacoFontOptions({ uiFont: "", editorFont: "", cjkFont: "", editorFontSize: 18 })).toMatchObject({
      fontSize: 18,
      lineHeight: 29
    });
  });
});
