import { Box, Gauge, Layers2 } from "lucide-react";
import { useEffect, useState } from "react";
import { AGENT_THINKING_OPTIONS, buildAgentModelChoices } from "../agent-runtime-options";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { useSubagentModelSettings } from "./use-subagent-model-settings";
import "./subagent-model-dialog.css";

type SubagentModelDialogProps = { open: boolean; onClose: () => void };

/**
 * 【Web】【子任务模型】配置共享默认模型、思考等级以及任务类型覆盖。
 * @param props 弹层打开状态和关闭回调
 * @returns 独立于主对话 Agent 档案的紧凑配置表单
 */
export function SubagentModelDialog({ open, onClose }: SubagentModelDialogProps) {
  const { t } = useI18n();
  const runtime = useSubagentModelSettings(open);
  const [target, setTarget] = useState("");
  const [modelSelection, setModelSelection] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState("auto");
  const [saved, setSaved] = useState(false);
  const profile = target
    ? runtime.settings?.profiles.find((item) => item.id === target)
    : runtime.settings?.defaults;
  const currentModel = profile?.provider_id && profile.model ? `${profile.provider_id}\t${profile.model}` : "";
  const currentThinking = profile?.thinking_level || "auto";

  useEffect(() => {
    setModelSelection(currentModel);
    setThinkingLevel(currentThinking);
  }, [open, target, currentModel, currentThinking]);

  const modelChoices = runtime.config
    ? buildAgentModelChoices(runtime.config, profile?.provider_id ?? "", profile?.model ?? "")
    : [];
  const unchanged = modelSelection === currentModel && thinkingLevel === currentThinking;
  const inherit = target ? t("Inherit shared defaults", "沿用共享默认值") : t("Inherit conversation settings", "沿用主对话设置");

  /** 保存当前目标的选择，无参数，返回保存完成信号；错误由表单展示。 */
  const save = async () => {
    if (!profile) return;
    const [providerId = "", model = ""] = modelSelection.split("\t");
    try {
      await runtime.save(target || null, { provider_id: providerId, model, thinking_level: thinkingLevel });
      setSaved(true);
    } catch {
      setSaved(false);
    }
  };

  return (
    <Modal
      open={open}
      title={t("Subagent models & thinking", "子任务模型与思考")}
      description={t("Configure Sai subagents. Saved settings apply to newly started subagents in Web and TUI.", "配置 Sai 子任务模型；保存后对 Web 和 TUI 新启动的子任务生效。")}
      size="small"
      onClose={onClose}
      footer={<>
        <Button onClick={onClose}>{t("Close", "关闭")}</Button>
        <Button variant="primary" onClick={() => void save()} disabled={!profile || unchanged || runtime.saving}>
          {runtime.saving ? t("Saving", "正在保存") : t("Save", "保存")}
        </Button>
      </>}
    >
      {runtime.loading && <div className="agent-quick-state">{t("Loading subagent settings", "正在读取子任务设置")}</div>}
      {runtime.error && <div className="agent-quick-error" role="alert">{runtime.error.message}</div>}
      {!runtime.loading && runtime.settings && (
        <div className="agent-quick-grid">
          <div className="agent-quick-field">
            <span><Layers2 size={14} strokeWidth={1.6} />{t("Applies to", "设置对象")}</span>
            <Select
              value={target}
              options={[
                { value: "", label: t("Shared defaults", "共享默认值") },
                ...runtime.settings.profiles.map((item) => ({ value: item.id, label: item.name }))
              ]}
              onChange={(value) => { setTarget(value); setSaved(false); runtime.clearError(); }}
              disabled={runtime.saving}
              ariaLabel={t("Subagent task type", "子任务类型")}
            />
          </div>
          <div className="grid min-w-0 gap-3 sm:grid-cols-[minmax(0,1fr)_8rem]">
            <div className="agent-quick-field min-w-0">
              <span><Box size={14} strokeWidth={1.6} />{t("Model", "模型")}</span>
              <Select
                value={modelSelection}
                options={[{ value: "", label: inherit }, ...modelChoices]}
                onChange={(value) => { setModelSelection(value); setSaved(false); }}
                disabled={runtime.saving}
                ariaLabel={t("Subagent model", "子任务模型")}
                menuPreferredWidth={360}
              />
            </div>
            <div className="agent-quick-field min-w-0">
              <span><Gauge size={14} strokeWidth={1.6} />{t("Thinking", "思考")}</span>
              <Select
                value={thinkingLevel}
                options={AGENT_THINKING_OPTIONS.map((option) => option.value === "auto" ? { ...option, label: inherit } : option)}
                onChange={(value) => { setThinkingLevel(value); setSaved(false); }}
                disabled={runtime.saving}
                ariaLabel={t("Subagent reasoning effort", "子任务思考等级")}
                menuPreferredWidth={240}
              />
            </div>
          </div>
          <p>{target
            ? t("Saving here takes priority over the Agent profile. Inherit shared defaults skips the profile model.", "此处保存的类型设置优先于 Agent 档案；选择沿用共享默认值会跳过档案模型。")
            : t("Task-specific settings and Agent profile models take priority over shared defaults. Select a task type above to override its profile.", "类型专用设置和 Agent 档案模型优先于共享默认值。若要覆盖档案模型，请先选择对应的子任务类型。")}</p>
          <p role="status">{saved
            ? t("Saved. New subagents will use these settings.", "已保存，后续启动的子任务将使用此设置。")
            : t("Shared across Web and TUI. Existing subagents keep their current settings.", "Web 与 TUI 共用此设置，已运行的子任务保留当前设置。")}</p>
        </div>
      )}
    </Modal>
  );
}
