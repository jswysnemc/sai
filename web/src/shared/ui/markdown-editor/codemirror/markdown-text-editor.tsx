import { Compartment, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { useEffect, useRef } from "react";
import { baseExtensions, presentationExtensions } from "./editor-extensions";
import "./markdown-text-editor.css";

type MarkdownTextEditorProps = {
  value: string;
  onChange: (value: string) => void;
  /** true 为所见即所得，false 为源码 */
  live: boolean;
  dark: boolean;
  readOnly?: boolean;
};

/**
 * 基于 CodeMirror 6 的 Markdown 文本编辑器。
 *
 * 文档本身始终是 Markdown 源码，所见即所得靠装饰层隐藏语法标记实现，
 * 因此模式之间往返不会改写用户的原始写法——这是与富文本方案的关键差别。
 *
 * @param props 文档内容、变更回调、模式、主题深浅与只读状态
 * @returns 编辑器容器
 */
export function MarkdownTextEditor({
  value,
  onChange,
  live,
  dark,
  readOnly = false,
}: MarkdownTextEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const presentationRef = useRef(new Compartment());
  onChangeRef.current = onChange;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    // 1. 变更回调经 ref 转发，父组件重渲染不会重建编辑器
    const view = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          ...baseExtensions((next) => onChangeRef.current(next)),
          presentationRef.current.of(presentationExtensions({ live, dark, readOnly })),
        ],
      }),
      parent: host,
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // 仅在挂载时创建实例，内容与配置由后续副作用同步
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    // 2. 模式、主题、只读状态变化时热替换，保留光标与撤销栈
    view.dispatch({
      effects: presentationRef.current.reconfigure(presentationExtensions({ live, dark, readOnly })),
    });
  }, [live, dark, readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === value) return;
    // 3. 外部内容变化（切换文件、外部重载）时整体替换，光标夹到新长度内
    const anchor = Math.min(view.state.selection.main.anchor, value.length);
    view.dispatch({
      changes: { from: 0, to: current.length, insert: value },
      selection: { anchor },
    });
  }, [value]);

  return <div className={live ? "markdown-cm live" : "markdown-cm source"} ref={hostRef} />;
}
