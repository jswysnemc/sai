import type { AppConfig } from "../../../api/contracts";
import { useSearchParams } from "react-router-dom";
import { SettingsPanel, SkSelect } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { buildVisibleAgentProfiles } from "./agent-profile-state";
import type { AgentOptions } from "./agents-types";
import "./agent-surface-defaults.css";

type AgentSurfaceDefaultsProps = {
  config: AppConfig;
  options: AgentOptions;
  onConfigChange: (config: AppConfig) => void;
};

type SurfaceField = "default_agent" | "tui_agent" | "cli_agent" | "gateway_agent";

/**
 * 配置 Web、TUI、CLI 和网关默认使用的 Agent。
 *
 * 四入口横排，说明作为悬停提示，避免窄列把中文竖排换行。
 *
 * @param props 应用配置、可选能力与更新回调
 * @returns 入口默认 Agent 面板
 */
export function AgentSurfaceDefaults({ config, options, onConfigChange }: AgentSurfaceDefaultsProps) {
  const { locale, t } = useI18n();
  const [, setParams] = useSearchParams();
  const surfaces: Array<{ field: SurfaceField; label: string; description: string }> = [
    { field: "default_agent", label: "Web", description: t("Used when the web workspace has no explicit Agent selection", "网页工作台未显式选择 Agent 时采用") },
    { field: "tui_agent", label: "TUI", description: t("Used when an interactive terminal session starts", "交互式终端会话启动时采用") },
    { field: "cli_agent", label: "CLI", description: t("Used for one-shot ask, message arguments, and Shell interception", "单次 ask、消息参数和 Shell 拦截采用") },
    { field: "gateway_agent", label: t("Gateway", "网关"), description: t("Used for QQ, WeChat, and other messaging gateway sessions", "QQ / 微信等消息网关会话采用") }
  ];
  const profiles = buildVisibleAgentProfiles(config.agents, options, config.subagent?.profiles, locale);
  const choices = profiles.map((profile) => ({
    value: profile.id,
    label: profile.name || profile.id
  }));

  /**
   * 更新入口默认档案并定位其详情。
   *
   * @param field 入口字段
   * @param value 档案标识
   * @returns 无返回值
   */
  const update = (field: SurfaceField, value: string) => {
    // 1. 【Agent】【入口默认】写回对应入口字段
    onConfigChange({ ...config, [field]: value === "default" ? null : value });
    // 2. 【Agent】【入口默认】定位到该档案详情
    setParams((previous) => {
      const next = new URLSearchParams(previous);
      next.set("item", value);
      return next;
    });
  };

  /**
   * 解析每种入口的默认档案。
   *
   * @param field 入口字段
   * @returns 有效档案标识
   */
  const valueOf = (field: SurfaceField) => {
    if (field === "default_agent") return config.default_agent ?? "default";
    if (field === "tui_agent") return config.tui_agent ?? "default";
    if (field === "cli_agent") return config.cli_agent ?? "default";
    return config.gateway_agent ?? "gateway";
  };

  return (
    <SettingsPanel title={t("Default Agent by entry point", "入口默认 Agent")}>
      <div className="agent-surface-defaults">
        {surfaces.map((surface) => (
          <label key={surface.field} className="agent-surface-item" title={surface.description}>
            <span>{surface.label}</span>
            <SkSelect
              value={valueOf(surface.field)}
              options={choices}
              ariaLabel={surface.description}
              onChange={(next) => update(surface.field, next)}
            />
          </label>
        ))}
      </div>
    </SettingsPanel>
  );
}
