import { forwardRef, useCallback, useImperativeHandle, useLayoutEffect, useRef, useState } from "react";
import type { ClipboardEvent, FormEvent, KeyboardEvent, PointerEvent } from "react";
import { useOutsidePointerDown } from "../../../shared/hooks/use-outside-pointer-down";
import {
  deleteAdjacentComposerAtom,
  insertEditorPlainText,
  readEditorTextSelection,
  renderComposerAtomEditor,
  selectComposerAtom,
  serializeComposerAtomEditor,
  setEditorTextSelection
} from "./composer-atom-editor";
import { findFileMentionTrigger, formatFileMention } from "./file-mention-token";
import { FileMentionPopover } from "./file-mention-popover";
import { filterSkills, SkillMentionPopover } from "./skill-mention-popover";
import type { SkillOption } from "./skill-mention-popover";
import { findSkillMentionTrigger, formatSkillMention } from "./skill-mention-token";
import { isCursorOnFirstLine, isCursorOnLastLine, navigateInputHistory } from "./input-history";
import type { InputHistoryState } from "./input-history";
import { useI18n } from "../../i18n/use-i18n";

type ComposerTextareaProps = {
  value: string;
  historyEntries: string[];
  disabled: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onPasteImages: (files: File[], selectionStart: number, selectionEnd: number) => Promise<number | undefined>;
  onSubmit: () => void;
};

export type ComposerTextareaHandle = {
  openMentionPicker: () => void;
};

/**
 * 把输入光标滚动到编辑器可视区域，并把消息列表滚到底部。
 *
 * @param editor 输入区根元素
 * @returns 无返回值
 */
function ensureComposerCaretVisible(editor: HTMLElement): void {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) {
    editor.scrollTop = editor.scrollHeight;
    return;
  }
  const range = selection.getRangeAt(0);
  if (!editor.contains(range.endContainer)) {
    editor.scrollTop = editor.scrollHeight;
    return;
  }
  const rect = range.getBoundingClientRect();
  const editorRect = editor.getBoundingClientRect();
  if (rect.height === 0 && rect.width === 0) {
    editor.scrollTop = editor.scrollHeight;
    return;
  }
  if (rect.bottom > editorRect.bottom - 4) {
    editor.scrollTop += rect.bottom - editorRect.bottom + 8;
  } else if (rect.top < editorRect.top + 4) {
    editor.scrollTop -= editorRect.top - rect.top + 8;
  }
}

/**
 * 渲染支持文件引用、skill 引用、图片粘贴和输入历史的富文本输入区。
 *
 * @param props 输入内容、历史记录、附件回调和提交回调
 * @param ref 暴露 openMentionPicker 的句柄
 * @returns 聊天文本输入区
 */
export const ComposerTextarea = forwardRef<ComposerTextareaHandle, ComposerTextareaProps>(function ComposerTextarea(props, ref) {
  const { t } = useI18n();
  const editorRef = useRef<HTMLDivElement>(null);
  const mentionPopoverRef = useRef<HTMLDivElement>(null);
  const skillPopoverRef = useRef<HTMLDivElement>(null);
  const historyRef = useRef<InputHistoryState>({ index: null, draft: "" });
  const lastEscapeRef = useRef(0);
  const pendingSelectionRef = useRef<{ start: number; end: number } | null>(null);
  const mentionRangeRef = useRef<{ start: number; end: number } | null>(null);
  const skillRangeRef = useRef<{ start: number; end: number; query: string } | null>(null);
  const skillOptionsRef = useRef<SkillOption[]>([]);
  const [mentionOpen, setMentionOpen] = useState(false);
  const [skillOpen, setSkillOpen] = useState(false);
  const [skillQuery, setSkillQuery] = useState("");
  const [skillActiveIndex, setSkillActiveIndex] = useState(0);

  useImperativeHandle(ref, () => ({
    /** 以当前选区为插入点打开文件引用浮层。 */
    openMentionPicker: () => {
      const editor = editorRef.current;
      const selection = editor ? readEditorTextSelection(editor) : null;
      skillRangeRef.current = null;
      setSkillOpen(false);
      mentionRangeRef.current = selection ?? { start: props.value.length, end: props.value.length };
      setMentionOpen(true);
    }
  }));

  useOutsidePointerDown(mentionPopoverRef, () => dismissMention(false), mentionOpen);
  useOutsidePointerDown(skillPopoverRef, () => dismissSkill(false), skillOpen);

  // 1. 外部状态变化时重建 token DOM，本地输入同步时不触碰浏览器选区
  useLayoutEffect(() => {
    const editor = editorRef.current;
    if (!editor || serializeComposerAtomEditor(editor) === props.value) return;
    renderComposerAtomEditor(editor, props.value);
    const pending = pendingSelectionRef.current;
    pendingSelectionRef.current = null;
    const start = pending?.start ?? props.value.length;
    const end = pending?.end ?? start;
    requestAnimationFrame(() => {
      editor.focus();
      setEditorTextSelection(editor, start, end);
      ensureComposerCaretVisible(editor);
    });
  }, [props.value]);

  /**
   * 关闭文件引用浮层。
   *
   * @param restoreFocus 是否把焦点交还输入区
   * @returns 无返回值
   */
  function dismissMention(restoreFocus: boolean): void {
    mentionRangeRef.current = null;
    setMentionOpen(false);
    if (restoreFocus) requestAnimationFrame(() => editorRef.current?.focus());
  }

  /**
   * 关闭 skill 浮层。
   *
   * @param restoreFocus 是否把焦点交还输入区
   * @returns 无返回值
   */
  function dismissSkill(restoreFocus: boolean): void {
    skillRangeRef.current = null;
    setSkillOpen(false);
    setSkillQuery("");
    setSkillActiveIndex(0);
    if (restoreFocus) requestAnimationFrame(() => editorRef.current?.focus());
  }

  /**
   * 在触发位置插入完整文件引用 token。
   *
   * @param path 选中的文件相对路径
   * @returns 无返回值
   */
  const handleMentionSelect = (path: string) => {
    const current = editorRef.current ? serializeComposerAtomEditor(editorRef.current) : props.value;
    const range = mentionRangeRef.current ?? { start: current.length, end: current.length };
    const mention = formatFileMention(path);
    const insertion = `${mention} `;
    const next = `${current.slice(0, range.start)}${insertion}${current.slice(range.end)}`;
    pendingSelectionRef.current = { start: range.start + insertion.length, end: range.start + insertion.length };
    props.onChange(next);
    dismissMention(false);
  };

  /**
   * 在触发位置插入 skill 引用 token。
   *
   * @param name 选中的 skill 名称
   * @returns 无返回值
   */
  const handleSkillSelect = useCallback((name: string) => {
    const current = editorRef.current ? serializeComposerAtomEditor(editorRef.current) : props.value;
    const range = skillRangeRef.current ?? { start: current.length, end: current.length, query: "" };
    const mention = formatSkillMention(name);
    const insertion = `${mention} `;
    const next = `${current.slice(0, range.start)}${insertion}${current.slice(range.end)}`;
    pendingSelectionRef.current = { start: range.start + insertion.length, end: range.start + insertion.length };
    props.onChange(next);
    dismissSkill(false);
  }, [props]);

  /** 缓存斜杠菜单选项，供键盘确认当前高亮项。 */
  const handleSkillOptionsChange = useCallback((options: SkillOption[]) => {
    skillOptionsRef.current = options;
  }, []);

  /**
   * 将 DOM 输入同步为后端文本，并识别 @ 文件引用与 / skill 引用。
   *
   * @param event 输入事件
   * @returns 无返回值
   */
  const handleInput = (event: FormEvent<HTMLDivElement>) => {
    historyRef.current = { index: null, draft: "" };
    const editor = event.currentTarget;
    const next = serializeComposerAtomEditor(editor);
    const selection = readEditorTextSelection(editor);
    const caret = selection?.end ?? next.length;
    const inputEvent = event.nativeEvent as InputEvent;
    const insertedText = inputEvent.inputType?.startsWith("insert") ? inputEvent.data : null;
    // 2. 优先识别 @ 文件引用
    const mentionRange = findFileMentionTrigger(next, caret, insertedText);
    if (mentionRange) {
      skillRangeRef.current = null;
      setSkillOpen(false);
      mentionRangeRef.current = mentionRange;
      setMentionOpen(true);
    } else {
      // 3. 识别 /skill 触发；URL 与路径中的 / 不会匹配
      const skillRange = findSkillMentionTrigger(next, caret);
      if (skillRange) {
        mentionRangeRef.current = null;
        setMentionOpen(false);
        skillRangeRef.current = skillRange;
        setSkillQuery(skillRange.query);
        setSkillOpen(true);
        setSkillActiveIndex(0);
      } else if (skillOpen) {
        dismissSkill(false);
      }
    }
    props.onChange(next);
    requestAnimationFrame(() => ensureComposerCaretVisible(editor));
  };

  /**
   * 粘贴图片附件或纯文本，禁止富文本节点进入编辑区。
   *
   * @param event 剪贴板事件
   * @returns 无返回值
   */
  const handlePaste = (event: ClipboardEvent<HTMLDivElement>) => {
    const editor = event.currentTarget;
    const files = Array.from(event.clipboardData.files).filter((file) => file.type.startsWith("image/"));
    const selection = readEditorTextSelection(editor) ?? { start: props.value.length, end: props.value.length };
    event.preventDefault();
    if (files.length > 0) {
      void props.onPasteImages(files, selection.start, selection.end).then((caret) => {
        if (caret === undefined) return;
        requestAnimationFrame(() => {
          setEditorTextSelection(editor, caret);
          ensureComposerCaretVisible(editor);
        });
      });
      return;
    }
    const text = event.clipboardData.getData("text/plain");
    if (text && insertEditorPlainText(editor, text)) {
      props.onChange(serializeComposerAtomEditor(editor));
      requestAnimationFrame(() => ensureComposerCaretVisible(editor));
    }
  };

  /**
   * 点击文件引用时选择完整 token，避免光标进入路径正文。
   *
   * @param event 指针按下事件
   * @returns 无返回值
   */
  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.focus();
    if (!selectComposerAtom(event.currentTarget, event.target)) return;
    event.preventDefault();
  };

  /**
   * 处理 skill 浮层导航、原子删除、输入历史、Escape 清空和回车提交。
   *
   * @param event 输入区键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const editor = event.currentTarget;
    const current = serializeComposerAtomEditor(editor);
    const selection = readEditorTextSelection(editor) ?? { start: current.length, end: current.length };

    if (skillOpen) {
      const filtered = filterSkills(skillOptionsRef.current, skillQuery);
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSkillActiveIndex((index) => Math.min(index + 1, Math.max(filtered.length - 1, 0)));
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSkillActiveIndex((index) => Math.max(index - 1, 0));
        return;
      }
      if ((event.key === "Enter" || event.key === "Tab") && !event.nativeEvent.isComposing) {
        const skill = filtered[skillActiveIndex];
        if (skill) {
          event.preventDefault();
          handleSkillSelect(skill.name);
          return;
        }
      }
      if (event.key === "Escape") {
        event.preventDefault();
        dismissSkill(true);
        return;
      }
    }

    if ((event.key === "Backspace" || event.key === "Delete") && selection.start === selection.end) {
      const direction = event.key === "Backspace" ? "backward" : "forward";
      if (deleteAdjacentComposerAtom(editor, direction)) {
        event.preventDefault();
        props.onChange(serializeComposerAtomEditor(editor));
        return;
      }
    }
    if (!skillOpen && event.key === "ArrowUp" && isCursorOnFirstLine(current, selection.start)) {
      if (applyHistoryNavigation("up", props, historyRef, pendingSelectionRef)) event.preventDefault();
      return;
    }
    if (!skillOpen && event.key === "ArrowDown" && isCursorOnLastLine(current, selection.end)) {
      if (applyHistoryNavigation("down", props, historyRef, pendingSelectionRef)) event.preventDefault();
      return;
    }
    if (event.key === "Escape") {
      const now = Date.now();
      if (mentionOpen) {
        event.preventDefault();
        dismissMention(true);
      } else if (now - lastEscapeRef.current < 500 && current) {
        event.preventDefault();
        props.onChange("");
        historyRef.current = { index: null, draft: "" };
      }
      lastEscapeRef.current = now;
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      props.onSubmit();
      return;
    }
    if (event.key === "Enter" && event.shiftKey) {
      event.preventDefault();
      if (insertEditorPlainText(editor, "\n")) {
        props.onChange(serializeComposerAtomEditor(editor));
        requestAnimationFrame(() => ensureComposerCaretVisible(editor));
      }
    }
  };

  return (
    <div className="composer-text-wrap">
      <FileMentionPopover ref={mentionPopoverRef} open={mentionOpen} onSelect={handleMentionSelect} onClose={() => dismissMention(true)} />
      <SkillMentionPopover
        ref={skillPopoverRef}
        open={skillOpen}
        query={skillQuery}
        activeIndex={skillActiveIndex}
        onActiveIndexChange={setSkillActiveIndex}
        onOptionsChange={handleSkillOptionsChange}
        onSelect={handleSkillSelect}
      />
      <div
        ref={editorRef}
        className="composer-editor"
        contentEditable={!props.disabled}
        suppressContentEditableWarning
        role="textbox"
        aria-label={t("Message input", "消息输入")}
        aria-multiline="true"
        aria-disabled={props.disabled}
        data-placeholder={props.placeholder}
        onInput={handleInput}
        onKeyDown={handleKeyDown}
        onPaste={handlePaste}
        onPointerDown={handlePointerDown}
      />
    </div>
  );
});

/**
 * 应用一次输入历史导航并安排光标位置。
 *
 * @param direction 历史移动方向
 * @param props 输入框属性
 * @param historyRef 历史游标引用
 * @param pendingSelectionRef 等待恢复的选区
 * @returns 是否切换了历史输入
 */
function applyHistoryNavigation(
  direction: "up" | "down",
  props: ComposerTextareaProps,
  historyRef: React.MutableRefObject<InputHistoryState>,
  pendingSelectionRef: React.MutableRefObject<{ start: number; end: number } | null>
): boolean {
  const result = navigateInputHistory(props.historyEntries, historyRef.current, props.value, direction);
  if (!result) return false;
  historyRef.current = result.state;
  const caret = direction === "up" ? 0 : result.value.length;
  pendingSelectionRef.current = { start: caret, end: caret };
  props.onChange(result.value);
  return true;
}
