import Editor, { loader, type OnMount } from "@monaco-editor/react";
import { useEffect, useRef, useState } from "react";
import { isDarkTheme, useTheme } from "../theme/theme";
import { configureMonacoEnvironment } from "./monaco-environment";
import { languageForPath } from "./editor-language";
import { FOCUS_COMPOSER_EVENT, INSERT_TERMINAL_SELECTION_EVENT } from "../chat/composer/composer-events";
import { useI18n } from "../i18n/use-i18n";
import type { EditorGitLine, EditorGitLineKind } from "./editor-git-decorations";

type MonacoCodeEditorProps = {
  path: string;
  value: string;
  onChange: (value: string) => void;
  /** 加载中占位文案 */
  loadingLabel: string;
  /** 相对 Git 基线的行级变更，渲染在行号右侧与概览标尺上 */
  gitLines?: EditorGitLine[];
};

/** 概览标尺上的变更标记色，与 gutter 装饰的主题色近似。 */
const OVERVIEW_RULER_COLORS: Record<EditorGitLineKind, string> = {
  added: "rgba(64, 175, 110, 0.8)",
  modified: "rgba(224, 158, 57, 0.8)",
  deleted: "rgba(229, 83, 75, 0.8)"
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
export function MonacoCodeEditor({ path, value, onChange, loadingLabel, gitLines }: MonacoCodeEditorProps) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const [ready, setReady] = useState(false);
  // 编辑器实例在 key=path 下随文件重建，用挂载版本号驱动装饰重挂
  const [mountVersion, setMountVersion] = useState(0);
  const areaRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const monacoRef = useRef<Parameters<OnMount>[1] | null>(null);
  const gitDecorationsRef = useRef<ReturnType<Parameters<OnMount>[0]["createDecorationsCollection"]> | null>(null);

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
      gitDecorationsRef.current = null;
    };
  }, [path]);

  useEffect(() => {
    // Git 行装饰：新增绿条、修改橙条、删除位置红色三角，另投影到概览标尺
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (!editor || !monaco) {
      return;
    }
    gitDecorationsRef.current?.clear();
    if (!gitLines || gitLines.length === 0) {
      return;
    }
    gitDecorationsRef.current = editor.createDecorationsCollection(
      gitLines.map((item) => ({
        range: new monaco.Range(item.line, 1, item.line, 1),
        options: {
          isWholeLine: true,
          linesDecorationsClassName: `editor-git-line editor-git-line-${item.kind}`,
          overviewRuler: {
            color: OVERVIEW_RULER_COLORS[item.kind],
            position: monaco.editor.OverviewRulerLane.Left
          }
        }
      }))
    );
    return () => {
      gitDecorationsRef.current?.clear();
    };
  }, [gitLines, mountVersion, path]);

  /**
   * 保存 Monaco 实例并立即按容器尺寸布局。
   *
   * @param editor Monaco 编辑器实例
   * @param monaco Monaco API
   * @returns 无
   */
  const handleMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;
    setMountVersion((version) => version + 1);
    const area = areaRef.current;
    if (area) editor.layout({ width: area.clientWidth, height: area.clientHeight });
    // 3. 把选区作为带来源的上下文原子发送到当前聊天输入区
    editor.addAction({
      id: "sai.send-selection-to-composer",
      label: t("Send selection to chat input", "发送选区到输入区"),
      precondition: "editorHasSelection",
      contextMenuGroupId: "9_cutcopypaste",
      contextMenuOrder: 4,
      run: (instance) => {
        const selection = instance.getSelection();
        const model = instance.getModel();
        if (!selection || selection.isEmpty() || !model) return;
        const content = model.getValueInRange(selection);
        window.dispatchEvent(new CustomEvent(INSERT_TERMINAL_SELECTION_EVENT, {
          detail: { source: path, content }
        }));
        window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT));
      }
    });
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
