import { useEffect, useRef, useState } from "react";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { TextInput } from "../../shared/ui/form/text-input";
import { useI18n } from "../i18n/use-i18n";
import type { BrowserDialog } from "./browser-protocol";

type BrowserDialogModalProps = {
  dialog: BrowserDialog | null;
  onAnswer: (accept: boolean, promptText?: string) => void;
};

/**
 * 页面 alert、confirm、prompt 与离开页面确认的回复弹层。
 *
 * 关闭弹层等同于取消；alert 只有确认按钮。
 *
 * @param props 当前对话框与回复回调
 * @returns 弹层
 */
export function BrowserDialogModal({ dialog, onAnswer }: BrowserDialogModalProps) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setValue(dialog?.default_prompt ?? "");
  }, [dialog]);

  if (!dialog) return null;
  const prompt = dialog.kind === "prompt";
  const alertOnly = dialog.kind === "alert";
  const title = dialog.kind === "beforeunload"
    ? t("Leave this page?", "离开此页面？")
    : t("Message from the page", "网页消息");
  let origin = dialog.url;
  try {
    origin = new URL(dialog.url).host || dialog.url;
  } catch {
    origin = dialog.url;
  }

  return (
    <Modal
      open
      size="small"
      title={title}
      description={origin}
      initialFocusRef={prompt ? inputRef : undefined}
      onClose={() => onAnswer(alertOnly)}
      footer={
        <>
          {!alertOnly && (
            <Button onClick={() => onAnswer(false)}>{t("Cancel", "取消")}</Button>
          )}
          <Button variant="primary" onClick={() => onAnswer(true, prompt ? value : undefined)}>
            {dialog.kind === "beforeunload" ? t("Leave", "离开") : t("OK", "确定")}
          </Button>
        </>
      }
    >
      <p className="browser-dialog-message">{dialog.message}</p>
      {prompt && (
        <form
          onSubmit={(event) => {
            event.preventDefault();
            onAnswer(true, value);
          }}
        >
          <TextInput
            ref={inputRef}
            value={value}
            aria-label={t("Response", "输入内容")}
            onChange={(event) => setValue(event.target.value)}
          />
        </form>
      )}
    </Modal>
  );
}
