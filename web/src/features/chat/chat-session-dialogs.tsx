import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";

type ChatSessionDialogsProps = {
  undoOpen: boolean;
  undoError: Error | null;
  onCloseUndo: () => void;
  onUndo: () => void;
  onCloseError: () => void;
};

/**
 * 集中渲染会话退回操作的确认和错误对话框。
 * @param props 对话框状态与会话操作回调
 * @returns 会话对话框集合
 */
export function ChatSessionDialogs(props: ChatSessionDialogsProps) {
  const { t } = useI18n();
  return (
    <>
      <Modal
        open={props.undoOpen}
        title={t("Step back to the previous turn?", "退回上一轮？")}
        description={t("The conversation moves back one turn. The original turn stays in the branch tree.", "对话位置退回一轮，原有内容仍保留在分支树中。")}
        size="small"
        onClose={props.onCloseUndo}
        footer={<><Button onClick={props.onCloseUndo}>{t("Cancel", "取消")}</Button><Button variant="primary" onClick={props.onUndo}>{t("Step back", "确认退回")}</Button></>}
      >
        <p>{t("Worktree changes are kept. The original message returns to the composer so you can revise and resend it.", "工作树修改会保留，原始消息将回到输入框，便于修改后重新发送。")}</p>
      </Modal>
      <Modal open={Boolean(props.undoError)} title={t("Undo failed", "撤销失败")} size="small" onClose={props.onCloseError} footer={<Button onClick={props.onCloseError}>{t("Close", "关闭")}</Button>}>
        <p>{props.undoError?.message}</p>
      </Modal>
    </>
  );
}
