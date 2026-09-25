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
  placeholder,
  rectangularSelection,
  type ViewUpdate,
} from "@codemirror/view";
import { headingFlashField } from "../outline/heading-jump";
import { outlineTracker, type OutlineListener } from "../outline/outline-tracker";
import { insertLink, isLinkUrl } from "./commands/inline-commands";
import { defaultImageUrl, editorDarkTheme, imageUrlResolver, type ImageUrlResolver } from "./editor-facets";
import { liveKeymap, sharedKeymap, type EditorKeymapOptions } from "./editor-keymap";
import { editorTheme } from "./editor-theme";
import { markdownMath } from "./markdown-math-syntax";
import { wysiwygBlockDecorations } from "./wysiwyg-block-decorations";
import { wysiwygDecorations } from "./wysiwyg-decorations";
import { wysiwygGuard } from "./wysiwyg-guard";
import { tableFocusPlugin } from "./wysiwyg-table-focus";

/** 与外部状态无关、但需要读取最新回调的依赖，全部以 getter 传入。 */
export type BaseExtensionOptions = {
  /** 文档变化回调 */
  onChange: (value: string) => void;
  /** 每次视图更新回调，供选区气泡等跟随选区的浮层刷新位置 */
  onUpdate: (update: ViewUpdate) => void;
  /** 快捷键依赖 */
  keymap: () => EditorKeymapOptions;
  /** 大纲订阅方 */
  outline: () => OutlineListener | null;
  /** 图片地址解析 */
  resolveImageUrl: () => ImageUrlResolver | undefined;
  /** 空文档占位文字 */
  placeholder: string;
};

/**
 * 装配与外部状态无关的静态扩展。
 *
 * @param options 回调与依赖，内部通过 getter 读取最新值
 * @returns CodeMirror 扩展数组
 */
export function baseExtensions(options: BaseExtensionOptions): Extension[] {
  return [
    // 1. 基础编辑能力：历史、选区绘制、输入缩进；软换行随展示状态热替换
    history(),
    drawSelection(),
    rectangularSelection(),
    indentOnInput(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    sharedKeymap(options.keymap),
    placeholder(options.placeholder),
    // 2. Markdown 语法解析：GFM 随 markdownLanguage 启用，另加数学公式；
    //    codeLanguages 让围栏代码块按语言标识做嵌套解析，语言包按需异步加载
    markdown({ base: markdownLanguage, codeLanguages: languages, extensions: [markdownMath] }),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    imageUrlResolver.of((src) => (options.resolveImageUrl() ?? defaultImageUrl)(src)),
    // 3. 大纲跟踪与跳转高亮
    outlineTracker(options.outline),
    headingFlashField,
    // 4. 选中文字后粘贴网址：直接生成链接
    EditorView.domEventHandlers({
      paste: (event, view) => {
        const text = event.clipboardData?.getData("text/plain") ?? "";
        if (view.state.selection.main.empty || !isLinkUrl(text)) return false;
        event.preventDefault();
        return insertLink(text.trim())(view);
      },
    }),
    // 5. 变更派发
    EditorView.updateListener.of((update) => {
      if (update.docChanged) options.onChange(update.state.doc.toString());
      options.onUpdate(update);
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
  /** 是否自动换行；缺省保持换行，避免未接入偏好的调用方改变排版 */
  wrap?: boolean;
};

/**
 * 装配随外部状态变化的扩展。
 *
 * 源码模式与预览模式共用同一个编辑器实例，靠这组扩展热替换来切换：
 * 前者显示行号与全部语法标记，后者隐藏标记并直接呈现排版，可就地编辑。
 * 共用实例使切换模式时光标位置与撤销栈都不丢失。
 *
 * @param options 模式、主题深浅、只读状态与换行
 * @returns CodeMirror 扩展数组
 */
export function presentationExtensions({ live, dark, readOnly, wrap = true }: PresentationOptions): Extension[] {
  return [
    ...(wrap ? [EditorView.lineWrapping] : []),
    editorTheme(dark, live),
    editorDarkTheme.of(dark),
    live
      ? [wysiwygBlockDecorations, wysiwygDecorations, wysiwygGuard, tableFocusPlugin, liveKeymap()]
      : [lineNumbers(), highlightActiveLineGutter()],
    EditorState.readOnly.of(readOnly),
    EditorView.editable.of(!readOnly),
  ];
}
