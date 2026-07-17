import { useEffect, useState } from "react";
import { Modal } from "../../../shared/ui/dialog/modal";
import { buildAgentChoices } from "../agent-options";
import { DefaultAgentPicker } from "./default-agent-picker";
import { readAgentConfigDraft, useAgentConfig, type AgentConfigDraft } from "./use-agent-config";
import "./agent-config-dialog.css";

type AgentConfigDialogProps = {
  open: boolean;
  onClose: () => void;
};

/**
 * 渲染 Agent 配置弹窗:设置全局默认 Agent 与子智能体统一模型。
 *
 * @param props 弹窗开合状态与关闭回调
 * @returns Agent 配置弹窗
 */
export function AgentConfigDialog({ open, onClose }: AgentConfigDialogProps) {
  const { config, isLoading, error, save, saving, saveError } = useAgentConfig();
  const [draft, setDraft] = useState<AgentConfigDraft | null>(null);

  useEffect(() => {
    if (open && config) setDraft(readAgentConfigDraft(config));
  }, [open, config]);

  const agentChoices = config ? buildAgentChoices(config) : [];

  /** 保存草稿并关闭弹窗。 */
  const handleSave = async () => {
    if (!draft) return;
    await save(draft);
    onClose();
  };

  return (
    <Modal
      open={open}
      title="Agent 配置"
      description="分别设置 Web、TUI、CLI 和网关默认使用的 Agent。"
      size="medium"
      onClose={onClose}
      footer={
        <div className="agent-config-footer">
          {(error || saveError) && <span className="agent-config-error">{(saveError ?? error)?.message}</span>}
          <button type="button" className="agent-config-cancel" onClick={onClose}>取消</button>
          <button type="button" className="agent-config-save" onClick={() => void handleSave()} disabled={!draft || saving}>
            {saving ? "保存中" : "保存"}
          </button>
        </div>
      }
    >
      {isLoading || !draft ? (
        <p className="agent-config-loading">正在读取配置</p>
      ) : (
        <div className="agent-config-body">
          <section className="agent-config-section">
            <h3>Web 默认 Agent</h3>
            <p>网页工作台未显式选择 Agent 时采用它。</p>
            <DefaultAgentPicker
              choices={agentChoices}
              value={draft.webAgent}
              onChange={(id) => setDraft({ ...draft, webAgent: id })}
            />
          </section>
          <section className="agent-config-section">
            <h3>TUI 默认 Agent</h3>
            <p>交互式终端会话启动时采用它。</p>
            <DefaultAgentPicker
              choices={agentChoices}
              value={draft.tuiAgent}
              onChange={(id) => setDraft({ ...draft, tuiAgent: id })}
            />
          </section>
          <section className="agent-config-section">
            <h3>CLI 默认 Agent</h3>
            <p>单次 ask、消息参数和 Shell 拦截运行采用它。</p>
            <DefaultAgentPicker
              choices={agentChoices}
              value={draft.cliAgent}
              onChange={(id) => setDraft({ ...draft, cliAgent: id })}
            />
          </section>
          <section className="agent-config-section">
            <h3>网关默认 Agent</h3>
            <p>QQ / 微信等消息网关会话采用它。</p>
            <DefaultAgentPicker
              choices={agentChoices}
              value={draft.gatewayAgent}
              onChange={(id) => setDraft({ ...draft, gatewayAgent: id })}
            />
          </section>
        </div>
      )}
    </Modal>
  );
}
