import { createContext, useContext, useRef, useState } from "react";
import { Modal } from "./modal";
import { useI18n } from "../../../features/i18n/use-i18n";
import "./dialog.css";

export type ConfirmOptions = {
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
};

type ConfirmRequest = ConfirmOptions & {
  resolve: (confirmed: boolean) => void;
};

const DialogContext = createContext<((options: ConfirmOptions) => Promise<boolean>) | null>(null);

/**
 * 提供全局确认对话框，并保证调用方使用异步结果。
 *
 * @param props 应用子节点
 * @returns 对话框上下文提供器
 */
export function DialogProvider({ children }: { children: React.ReactNode }) {
  const { t } = useI18n();
  const [request, setRequest] = useState<ConfirmRequest | null>(null);
  const activeRequest = useRef<ConfirmRequest | null>(null);

  /** 创建一项确认请求。 */
  const confirm = (options: ConfirmOptions) => new Promise<boolean>((resolve) => {
    activeRequest.current?.resolve(false);
    const next = { ...options, resolve };
    activeRequest.current = next;
    setRequest(next);
  });

  /** 完成当前确认请求。 */
  const settle = (confirmed: boolean) => {
    const current = activeRequest.current;
    activeRequest.current = null;
    setRequest(null);
    current?.resolve(confirmed);
  };

  return (
    <DialogContext.Provider value={confirm}>
      {children}
      <Modal
        open={Boolean(request)}
        title={request?.title ?? t("Confirm action", "确认操作")}
        description={request?.description}
        size="small"
        onClose={() => settle(false)}
        footer={(
          <>
            <button type="button" className="ui-button secondary" onClick={() => settle(false)}>{request?.cancelLabel ?? t("Cancel", "取消")}</button>
            <button type="button" className={request?.danger ? "ui-button danger" : "ui-button primary"} onClick={() => settle(true)}>{request?.confirmLabel ?? t("Confirm", "确认")}</button>
          </>
        )}
      >
        <div className="confirm-dialog-detail">{t("The action runs immediately after confirmation.", "操作确认后立即执行。")}</div>
      </Modal>
    </DialogContext.Provider>
  );
}

/**
 * 返回全局确认对话框方法。
 *
 * @returns 异步确认方法
 */
export function useConfirm() {
  const confirm = useContext(DialogContext);
  if (!confirm) throw new Error("DialogProvider is missing");
  return confirm;
}
