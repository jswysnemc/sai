import { useState } from "react";
import { Pencil } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { QuestionAnswerInput } from "./question-answer-input";
import type { Translate } from "./question-card-state";

type QuestionCustomAnswerProps = {
  draft: string;
  onDraft: (value: string) => void;
  onSave: (images: string[]) => void;
  attachmentKey: string;
  allowImages: boolean;
  openInitially?: boolean;
  t: Translate;
};

/**
 * 渲染折叠在选项下方的自定义补充回答。
 *
 * 没有预设选项时直接展开共享编辑器，回车提交，Shift+Enter 换行。
 *
 * @param props 草稿内容、草稿更新与保存回调
 * @returns 紧凑的自定义回答区
 */
export function QuestionCustomAnswer({ draft, onDraft, onSave, attachmentKey, allowImages, openInitially, t }: QuestionCustomAnswerProps) {
  const [open, setOpen] = useState(() => Boolean(openInitially) || draft.trim().length > 0);

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
    <QuestionAnswerInput draft={draft} onDraft={onDraft} onSave={onSave} attachmentKey={attachmentKey} allowImages={allowImages} t={t} />
  );
}
