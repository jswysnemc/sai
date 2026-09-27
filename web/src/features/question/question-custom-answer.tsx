import { useState, type KeyboardEvent } from "react";
import { Pencil } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { TextInput } from "../../shared/ui/form/text-input";
import type { Translate } from "./question-card-state";

type QuestionCustomAnswerProps = {
  draft: string;
  onDraft: (value: string) => void;
  onSave: () => void;
  t: Translate;
};

/**
 * 渲染折叠在选项下方的自定义补充回答。
 *
 * 默认只显示一行文字入口；展开后是单行输入与"使用"按钮，回车等同于点击使用。
 *
 * @param props 草稿内容、草稿更新与保存回调
 * @returns 紧凑的自定义回答区
 */
export function QuestionCustomAnswer({ draft, onDraft, onSave, t }: QuestionCustomAnswerProps) {
  const [open, setOpen] = useState(() => draft.trim().length > 0);

  /**
   * 回车提交草稿，输入法组字过程中不提交。
   *
   * @param event 输入框键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
    event.preventDefault();
    onSave();
  };

  if (!open) {
    return (
      <Button variant="ghost" size="small" className="question-custom-toggle" onClick={() => setOpen(true)}>
        <span className="icon-label">
          <Pencil size={12} aria-hidden />
          <span>{t("Other answer", "其他回答")}</span>
        </span>
      </Button>
    );
  }

  return (
    <div className="question-custom">
      <TextInput
        autoFocus
        value={draft}
        onChange={(event) => onDraft(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={t("Enter another answer", "输入其他回答")}
        aria-label={t("Custom answer", "自定义回答")}
      />
      <Button size="small" disabled={!draft.trim()} onClick={onSave}>
        {t("Use", "使用")}
      </Button>
    </div>
  );
}
