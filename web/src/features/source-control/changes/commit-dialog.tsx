import type { ComponentProps } from "react";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import { CommitControl } from "./commit-control";

type CommitDialogProps = ComponentProps<typeof CommitControl> & {
  open: boolean;
  onClose: () => void;
};

/**
 * 把提交说明和提交动作放进弹层，面板本身不再常驻这条输入。
 *
 * @param props 弹层开关与提交控件参数
 * @returns 提交弹层
 */
export function CommitDialog(props: CommitDialogProps) {
  const { t } = useI18n();
  const { open, onClose, onCommit, ...control } = props;

  return (
    <Modal
      open={open}
      title={t("Commit", "提交")}
      description={t("Write a message, then commit. Ctrl+Enter commits from the field.", "写好说明后提交。在输入框里按 Ctrl+Enter 也会提交。")}
      size="small"
      className="git-commit-dialog"
      onClose={onClose}
    >
      <CommitControl
        {...control}
        onCommit={async (options) => {
          const succeeded = await onCommit(options);
          if (succeeded) onClose();
          return succeeded;
        }}
      />
    </Modal>
  );
}
