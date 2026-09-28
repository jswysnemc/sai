import Editor, { loader } from "@monaco-editor/react";
import { Braces, WandSparkles } from "../icons";
import { useEffect, useState } from "react";
import { isDarkTheme, useTheme } from "../../../features/theme/theme";
import { configureMonacoEnvironment } from "../../../features/workspace/monaco-environment";
import "./json-code-editor.css";
import { useI18n } from "../../../features/i18n/use-i18n";
import { Button } from "../button/button";

/** 编辑器对外提供的错误位置。 */
export type JsonEditorDiagnostic = { message: string; line: number; column: number };

type JsonCodeEditorProps = {
  value: string;
  height?: number | string;
  ariaLabel?: string;
  onChange: (value: string) => void;
  onDiagnostics?: (diagnostics: JsonEditorDiagnostic[]) => void;
  reveal?: { line: number; column: number; request: number };
};

/**
 * 渲染带语法着色、校验和格式化操作的 JSON 编辑器。
 *
 * @param props JSON 文本、高度和更新回调
 * @returns Monaco JSON 编辑器
 */
export function JsonCodeEditor({ value, height = 420, ariaLabel, onChange, onDiagnostics, reveal }: JsonCodeEditorProps) {
  const { t } = useI18n();
  const resolvedAriaLabel = ariaLabel ?? t("JSON editor", "JSON 编辑器");
  const { theme } = useTheme();
  const [ready, setReady] = useState(false);
  const [editor, setEditor] = useState<import("monaco-editor").editor.IStandaloneCodeEditor | null>(null);

  useEffect(() => {
    let active = true;
    // 1. 先注册 Worker 与 _VSCODE_FILE_ROOT，再加载 Monaco 主模块
    configureMonacoEnvironment();
    import("monaco-editor").then((monaco) => {
      loader.config({ monaco });
      if (active) setReady(true);
    });
    return () => { active = false; };
  }, []);

  const dark = isDarkTheme(theme);
  useEffect(() => {
    if (!editor || !reveal) return;
    editor.setPosition({ lineNumber: reveal.line, column: reveal.column });
    editor.revealLineInCenter(reveal.line);
    editor.focus();
  }, [editor, reveal]);

  return (
    <div className="json-code-editor" aria-label={resolvedAriaLabel}>
      <header><span><Braces size={14} />JSON</span><Button variant="ghost" size="small" onClick={() => void editor?.getAction("editor.action.formatDocument")?.run()} disabled={!editor}><WandSparkles size={14} />{t("Format", "格式化")}</Button></header>
      <div className="json-editor-surface" style={{ height }}>
        {ready ? <Editor language="json" value={value} theme={dark ? "vs-dark" : "light"} onChange={(next) => onChange(next ?? "")} onMount={(instance) => setEditor(instance)} onValidate={(markers) => onDiagnostics?.(markers.filter((marker) => marker.severity >= 8).map((marker) => ({ message: marker.message, line: marker.startLineNumber, column: marker.startColumn })))} options={{ automaticLayout: true, minimap: { enabled: false }, fontFamily: "Fira Code", fontSize: 12, lineHeight: 20, scrollBeyondLastLine: false, folding: true, bracketPairColorization: { enabled: true }, formatOnPaste: true, padding: { top: 10, bottom: 10 }, ariaLabel: resolvedAriaLabel }} /> : <div className="editor-state">{t("Loading JSON editor", "加载 JSON 编辑器")}</div>}
      </div>
    </div>
  );
}
