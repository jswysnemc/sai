import type { AppConfig } from "../../../api/contracts";
import { ContextBudgetPreview } from "./context-budget-preview";
import { useI18n } from "../../i18n/use-i18n";
import { CompactionModelField } from "../compaction-model-field";
import { FieldGrid, SettingsField, SettingsPanel, SkNumberInput, SwitchField } from "../kit";
import { MemoryExtractionModelField } from "./session-memory-extraction-field";
import type { RuntimeSettingsProps } from "./runtime-settings-types";

/**
 * 【Web 设置】【上下文预算】配置新会话默认预算、压缩阈值与辅助模型。
 * @param props 应用配置与更新回调
 * @returns 上下文管理面板
 */
export function ContextSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const context = { default_max_chars: 120_000, ...config.context };

  /**
   * 合并上下文字段并保留未编辑配置。
   * @param patch 修改的上下文字段
   * @returns 无返回值
   */
  const update = (patch: Partial<NonNullable<AppConfig["context"]>>) => onConfigChange({ ...config, context: { ...context, ...patch } });

  return (
    <SettingsPanel title={t("Context management", "上下文管理")} description={t("Defaults for new sessions. The chat context panel can override ratio and reserve for an individual session.", "新会话使用这些默认值；对话上下文面板可单独修改当前会话的比例与预留。")}>
      <FieldGrid columns={3}>
        <SettingsField label={t("Default context tokens", "默认上下文 token 数")} hint={t("Used when the model has no dedicated context window.", "模型未单独配置上下文窗口时使用。")} configKey="context.default_max_chars" anchor="runtime.context.default_max_chars" size="sm">
          <SkNumberInput value={context.default_max_chars} min={1} integer onChange={(value) => update({ default_max_chars: value ?? 120_000 })} />
        </SettingsField>
        <SettingsField label={t("Auto-compact ratio", "自动压缩比例")} hint={t("Compact when this percentage of the context window is used.", "上下文用量达到此比例时自动压缩。")} configKey="context.compaction_ratio" anchor="runtime.context.compaction_ratio" size="xs">
          <SkNumberInput value={Math.round((context.compaction_ratio ?? 0.9) * 100)} min={50} max={99} integer unit="%" onChange={(value) => update({ compaction_ratio: (value ?? 90) / 100 })} />
        </SettingsField>
        <SettingsField label={t("Reserved headroom", "压缩预留 token")} hint={t("Large windows compact at this remaining budget. 0 uses the ratio only; small windows always follow the ratio.", "大窗口剩余量低于此值时压缩。0 表示只按比例；小窗口始终按比例。")} configKey="context.compaction_reserve_tokens" anchor="runtime.context.compaction_reserve_tokens" size="sm">
          <SkNumberInput value={context.compaction_reserve_tokens ?? 50_000} min={0} integer step={1000} onChange={(value) => update({ compaction_reserve_tokens: value ?? 0 })} />
        </SettingsField>
        <CompactionModelField config={config} onConfigChange={onConfigChange} />
        <MemoryExtractionModelField config={config} onConfigChange={onConfigChange} />
      </FieldGrid>
      <ContextBudgetPreview limit={context.default_max_chars ?? 120_000} ratio={context.compaction_ratio ?? 0.9} reserve={context.compaction_reserve_tokens ?? 50_000} />
      <SwitchField
        label={t("Experimental tool-result compression", "实验性工具结果压缩")}
        hint={t("The model summarizes older tool results; originals remain available for retrieval. Requires the built-in engine and enabled context tools. Global compaction remains active.", "由模型摘要较早的工具结果，原文保留供回读。需要内置引擎并启用上下文工具；原有全局压缩继续生效。")}
        checked={context.experimental_context_blocks ?? false}
        onChange={(enabled) => update({ experimental_context_blocks: enabled })}
        configKey="context.experimental_context_blocks"
        anchor="runtime.context.experimental_context_blocks"
      />
    </SettingsPanel>
  );
}
