import { useId, useRef, useState } from "react";
import { toDisplayError } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { TextInput } from "../../shared/ui/form/text-input";
import { useI18n } from "../i18n/use-i18n";

/**
 * 在独立对话框中编辑会话标题，保存失败时保留输入。
 * @param props 当前会话标识、标题和保存、关闭回调
 * @returns 重命名对话框
 */
export function SessionRenameDialog({ session, onRename, onClose }: {
  session: { id: string; title: string };
  onRename: (id: string, title: string) => Promise<void>;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(session.title);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const formId = useId();
  const inputId = useId();
  const errorId = useId();

  /** 提交有效标题并在成功后关闭。@returns 保存完成信号 */
  const save = async () => {
    const title = draft.trim();
    if (!title || pending) return;
    if (title === session.title) { onClose(); return; }
    setPending(true);
    setError(null);
    try {
      await onRename(session.id, title);
      onClose();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to rename session", "重命名会话失败"));
    } finally {
      setPending(false);
    }
  };

  return <Modal open title={t("Rename session", "重命名会话")} size="small" initialFocusRef={inputRef} onClose={() => { if (!pending) onClose(); }} footer={<>
    <Button disabled={pending} onClick={onClose}>{t("Cancel", "取消")}</Button>
    <Button variant="primary" type="submit" form={formId} disabled={pending || !draft.trim()}>{pending ? t("Saving", "正在保存") : t("Save", "保存")}</Button>
  </>}>
    <form id={formId} className="grid gap-3" onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <label htmlFor={inputId} className="text-sm">{t("Session name", "会话名称")}</label>
      <TextInput ref={inputRef} id={inputId} value={draft} onChange={(event) => setDraft(event.target.value)} onFocus={(event) => event.currentTarget.select()} disabled={pending} autoComplete="off" aria-describedby={error ? errorId : undefined} />
      {error && <p id={errorId} role="alert" className="m-0 text-sm text-[var(--danger)]">{error.message}</p>}
    </form>
  </Modal>;
}
