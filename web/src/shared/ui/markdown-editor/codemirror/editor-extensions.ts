import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { defaultHighlightStyle, indentOnInput, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";
import { editorTheme } from "./editor-theme";
import { wysiwygBlockDecorations } from "./wysiwyg-block-decorations";
import { wysiwygDecorations } from "./wysiwyg-decorations";

/**
 * 装配与外部状态无关的静态扩展。
 *
 * @param onChange 文档变化回调，内部通过闭包读取最新引用
 * @returns CodeMirror 扩展数组
 */
export function baseExtensions(onChange: (value: string) => void): Extension[] {
  return [
    // 1. 基础编辑能力：历史、选区绘制、输入缩进、软换行
    history(),
    drawSelection(),
    rectangularSelection(),
    indentOnInput(),
    EditorView.lineWrapping,
    keymap.of([...defaultKeymap, ...historyKeymap]),
    // 2. Markdown 语法解析，GFM 扩展随 markdownLanguage 一并启用；
    //    codeLanguages 让围栏代码块按语言标识做嵌套解析，语言包按需异步加载
    markdown({ base: markdownLanguage, codeLanguages: languages }),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    // 3. 变更派发
    EditorView.updateListener.of((update) => {
      if (update.docChanged) onChange(update.state.doc.toString());
    }),
  ];
}

type PresentationOptions = {
  /** 是否启用所见即所得装饰 */
  live: boolean;
  /** 是否为深色主题 */
  dark: boolean;
  /** 是否只读 */
  readOnly: boolean;
};

/**
 * 装配随外部状态变化的扩展。
 *
 * 源码模式与所见即所得模式共用同一个编辑器实例，靠这组扩展热替换来切换：
 * 前者显示行号与全部语法标记，后者隐藏标记并直接呈现排版。
 * 共用实例使切换模式时光标位置与撤销栈都不丢失。
 *
 * @param options 模式、主题深浅与只读状态
 * @returns CodeMirror 扩展数组
 */
export function presentationExtensions({ live, dark, readOnly }: PresentationOptions): Extension[] {
  return [
    editorTheme(dark, live),
    live
      ? [wysiwygDecorations, wysiwygBlockDecorations]
      : [lineNumbers(), highlightActiveLineGutter()],
    EditorState.readOnly.of(readOnly),
    EditorView.editable.of(!readOnly),
  ];
}
