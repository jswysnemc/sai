import { useEffect, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { MarkdownEditor } from "../../../shared/ui/markdown-editor/markdown-editor";
import { MarkdownModeToggle } from "../../../shared/ui/markdown-editor/markdown-mode-toggle";
import { useMarkdownMode } from "../../../shared/ui/markdown-editor/use-markdown-mode";
import { isDarkTheme, useTheme } from "../../theme/theme";
import { useI18n } from "../../i18n/use-i18n";

type AgentPromptEditorDialogProps = {
  open: boolean;
  value: string;
  onClose: () => void;
  onApply: (value: string) => void;
};

/**
 * 在大尺寸弹窗里编辑 Agent 系统提示词。
 *
 * 打开时复制当前正文为草稿，取消丢弃草稿，确认后才写回档案。
 *
 * @param props 开关、当前正文、关闭与应用回调
 * @returns 提示词编辑弹窗
 */
export function AgentPromptEditorDialog({ open, value, onClose, onApply }: AgentPromptEditorDialogProps) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const [mode, setMode] = useMarkdownMode();
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    if (!open) return;
    setDraft(value);
  }, [open, value]);

  /**
   * 把草稿写回档案并关闭弹窗。
   *
   * @returns 无返回值
   */
  const applyDraft = () => {
    onApply(draft);
    onClose();
  };

  return (
    <Modal
      open={open}
      className="agent-prompt-dialog"
      size="large"
      title={t("Edit system prompt", "编辑系统提示词")}
      description={t(
        "Stable role constraints only. Built-in sections stay on the profile and are appended at runtime.",
        "只写长期稳定的角色约束。内置分段仍留在档案里，运行时追加到这段提示词之后。"
      )}
      onClose={onClose}
      footer={(
        <>
          <Button variant="secondary" onClick={onClose}>{t("Cancel", "取消")}</Button>
          <Button variant="primary" onClick={applyDraft}>{t("Apply", "确认应用")}</Button>
        </>
      )}
    >
      <div className="agent-prompt-dialog-body">
        <MarkdownModeToggle mode={mode} onChange={setMode} t={t} />
        <MarkdownEditor
          value={draft}
          onChange={setDraft}
          mode={mode}
          onModeChange={setMode}
          dark={isDarkTheme(theme)}
        />
      </div>
    </Modal>
  );
}
