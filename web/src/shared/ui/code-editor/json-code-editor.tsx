import Editor, { loader } from "@monaco-editor/react";
import { Braces, WandSparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useTheme } from "../../../features/theme/theme";
import "./json-code-editor.css";
import { useI18n } from "../../../features/i18n/use-i18n";

type JsonCodeEditorProps = {
  value: string;
  height?: number | string;
  ariaLabel?: string;
  onChange: (value: string) => void;
};

/**
 * 渲染带语法着色、校验和格式化操作的 JSON 编辑器。
 *
 * @param props JSON 文本、高度和更新回调
 * @returns Monaco JSON 编辑器
 */
export function JsonCodeEditor({ value, height = 420, ariaLabel, onChange }: JsonCodeEditorProps) {
  const { t } = useI18n();
  const resolvedAriaLabel = ariaLabel ?? t("JSON editor", "JSON 编辑器");
  const { theme } = useTheme();
  const [ready, setReady] = useState(false);
  const [editor, setEditor] = useState<import("monaco-editor").editor.IStandaloneCodeEditor | null>(null);

  useEffect(() => {
    let active = true;
    import("monaco-editor").then((monaco) => {
      loader.config({ monaco });
      if (active) setReady(true);
    });
    return () => { active = false; };
  }, []);

  const dark = theme === "graphite" || theme === "ocean" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  return (
    <div className="json-code-editor" aria-label={resolvedAriaLabel}>
      <header><span><Braces size={13} />JSON</span><button type="button" onClick={() => void editor?.getAction("editor.action.formatDocument")?.run()} disabled={!editor}><WandSparkles size={13} />{t("Format", "格式化")}</button></header>
      <div className="json-editor-surface" style={{ height }}>
        {ready ? <Editor language="json" value={value} theme={dark ? "vs-dark" : "light"} onChange={(next) => onChange(next ?? "")} onMount={(instance) => setEditor(instance)} options={{ automaticLayout: true, minimap: { enabled: false }, fontFamily: "Fira Code", fontSize: 12, lineHeight: 20, scrollBeyondLastLine: false, folding: true, bracketPairColorization: { enabled: true }, formatOnPaste: true, padding: { top: 10, bottom: 10 }, ariaLabel: resolvedAriaLabel }} /> : <div className="editor-state">{t("Loading JSON editor", "加载 JSON 编辑器")}</div>}
      </div>
    </div>
  );
}
