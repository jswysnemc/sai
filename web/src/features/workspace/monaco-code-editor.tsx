import Editor, { loader, type OnMount } from "@monaco-editor/react";
import { useEffect, useRef, useState } from "react";
import { isDarkTheme, useTheme } from "../theme/theme";
import { configureMonacoEnvironment } from "./monaco-environment";
import { languageForPath } from "./editor-language";

type MonacoCodeEditorProps = {
  path: string;
  value: string;
  onChange: (value: string) => void;
  /** 加载中占位文案 */
  loadingLabel: string;
};

/**
 * Monaco 代码编辑器封装。
 *
 * 负责 Monaco 主模块的按需加载与容器尺寸同步：Monaco 的 automaticLayout 在
 * 网格拖动场景下会沿用旧宽度，这里改由 ResizeObserver 主动通知实际尺寸。
 *
 * @param props 文件路径、内容、变更回调与加载文案
 * @returns 编辑器区域
 */
export function MonacoCodeEditor({ path, value, onChange, loadingLabel }: MonacoCodeEditorProps) {
  const { theme } = useTheme();
  const [ready, setReady] = useState(false);
  const areaRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

  useEffect(() => {
    let active = true;
    // 1. 先注册语言 Worker，再加载 Monaco 主模块
    configureMonacoEnvironment();
    import("monaco-editor").then((monaco) => {
      loader.config({ monaco });
      if (active) setReady(true);
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const container = areaRef.current;
    if (!container) return;
    let frame = 0;
    // 2. 用实际编辑区域尺寸通知 Monaco，避免拖动网格时沿用旧宽度
    const observer = new ResizeObserver(([entry]) => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const width = Math.max(0, Math.floor(entry.contentRect.width));
        const height = Math.max(0, Math.floor(entry.contentRect.height));
        if (width > 0 && height > 0) editorRef.current?.layout({ width, height });
      });
    });
    observer.observe(container);
    return () => {
      window.cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [ready, path]);

  useEffect(() => {
    return () => {
      editorRef.current = null;
    };
  }, [path]);

  /**
   * 保存 Monaco 实例并立即按容器尺寸布局。
   *
   * @param editor Monaco 编辑器实例
   * @returns 无
   */
  const handleMount: OnMount = (editor) => {
    editorRef.current = editor;
    const area = areaRef.current;
    if (area) editor.layout({ width: area.clientWidth, height: area.clientHeight });
  };

  return (
    <div className="monaco-code-editor" ref={areaRef}>
      {ready ? (
        <Editor
          key={path}
          path={path}
          language={languageForPath(path)}
          value={value}
          width="100%"
          height="100%"
          onMount={handleMount}
          onChange={(next) => onChange(next ?? "")}
          theme={isDarkTheme(theme) ? "vs-dark" : "light"}
          options={{
            minimap: { enabled: false },
            fontFamily: "Fira Code",
            fontSize: 13,
            lineHeight: 21,
            padding: { top: 12 },
            automaticLayout: false,
            scrollBeyondLastLine: false,
          }}
        />
      ) : (
        <div className="editor-state">{loadingLabel}</div>
      )}
    </div>
  );
}
