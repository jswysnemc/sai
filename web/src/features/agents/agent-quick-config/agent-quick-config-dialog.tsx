import { BrainCircuit, Cpu } from "lucide-react";
import { useEffect, useState } from "react";
import type { AgentChoice } from "../agent-types";
import { AGENT_THINKING_OPTIONS, buildAgentModelChoices } from "../agent-runtime-options";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { useAgentRuntimeConfig } from "./use-agent-runtime-config";
import "./agent-quick-config.css";

type AgentQuickConfigDialogProps = {
  open: boolean;
  agent: AgentChoice | null;
  onClose: () => void;
};

/**
 * 渲染输入区当前 Agent 的模型与思考等级快速配置弹层。
 *
 * @param props 弹层状态、当前 Agent 和关闭回调
 * @returns 紧凑运行参数表单
 */
export function AgentQuickConfigDialog({ open, agent, onClose }: AgentQuickConfigDialogProps) {
  const { t } = useI18n();
  const runtime = useAgentRuntimeConfig(open);
  const profile = runtime.profiles.find((item) => item.id === agent?.id) ?? null;
  const [modelSelection, setModelSelection] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState("auto");

  useEffect(() => {
    if (!open || !profile) return;
    setModelSelection(profile.provider_id && profile.model ? `${profile.provider_id}\t${profile.model}` : "");
    setThinkingLevel(profile.thinking_level || "auto");
  }, [open, profile?.id, profile?.provider_id, profile?.model, profile?.thinking_level]);

  const modelChoices = runtime.config
    ? buildAgentModelChoices(runtime.config, profile?.provider_id ?? "", profile?.model ?? "")
    : [];
  const unchanged = profile
    ? modelSelection === (profile.provider_id && profile.model ? `${profile.provider_id}\t${profile.model}` : "")
      && thinkingLevel === (profile.thinking_level || "auto")
    : true;

  /** 保存当前 Agent 的模型和思考等级。 */
  const save = async () => {
    if (!agent || !profile) return;
    const [providerId = "", model = ""] = modelSelection.split("\t");
    await runtime.save(agent.id, {
      provider_id: providerId,
      model,
      thinking_level: thinkingLevel
    });
    onClose();
  };

  return (
    <Modal
      open={open}
      title={agent ? t(`Configure ${agent.name}`, `配置 ${agent.name}`) : t("Agent quick settings", "Agent 快速配置")}
      description={t("Adjust only the selected Agent's model and reasoning effort.", "仅调整当前 Agent 的模型和思考等级。")}
      size="small"
      onClose={onClose}
      footer={<>
        <Button onClick={onClose}>{t("Cancel", "取消")}</Button>
        <Button variant="primary" onClick={() => void save()} disabled={!profile || unchanged || runtime.saving}>
          {runtime.saving ? t("Saving", "正在保存") : t("Save", "保存")}
        </Button>
      </>}
    >
      {runtime.loading && <div className="agent-quick-state">{t("Loading Agent settings", "正在读取 Agent 配置")}</div>}
      {!runtime.loading && runtime.error && <div className="agent-quick-error">{runtime.error.message}</div>}
      {!runtime.loading && !runtime.error && !profile && (
        <div className="agent-quick-state">
          {t("The default Agent inherits the model and reasoning effort selected in the composer.", "默认 Agent 沿用输入区当前选择的模型和思考等级。")}
        </div>
      )}
      {!runtime.loading && !runtime.error && profile && (
        <div className="agent-quick-grid">
          <div className="agent-quick-field">
            <span><Cpu size={14} />{t("Agent model", "Agent 模型")}</span>
            <Select
              value={modelSelection}
              options={[
                { value: "", label: t("Inherit the composer model", "沿用输入区模型") },
                ...modelChoices
              ]}
              onChange={setModelSelection}
              ariaLabel={t("Agent model", "Agent 模型")}
              menuPreferredWidth={360}
            />
          </div>
          <div className="agent-quick-field">
            <span><BrainCircuit size={14} />{t("Reasoning effort", "思考等级")}</span>
            <Select
              value={thinkingLevel}
              options={AGENT_THINKING_OPTIONS}
              onChange={setThinkingLevel}
              ariaLabel={t("Agent reasoning effort", "Agent 思考等级")}
              menuPreferredWidth={240}
            />
          </div>
          <p>{t("Changes apply to this Agent on Web, TUI, CLI, and gateway runs.", "修改会应用到该 Agent 的 Web、TUI、CLI 和网关运行。")}</p>
        </div>
      )}
    </Modal>
  );
}
