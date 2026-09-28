import type { AppConfig } from "../../../api/contracts";
import { SettingsField, SettingsPanel, SkSelect } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { buildVisibleAgentProfiles } from "./agent-profile-state";
import type { AgentOptions } from "./agents-types";

type AgentSurfaceDefaultsProps = {
  config: AppConfig;
  options: AgentOptions;
  onConfigChange: (config: AppConfig) => void;
};

type SurfaceField = "default_agent" | "tui_agent" | "cli_agent" | "gateway_agent";

/**
 * 配置 Web、TUI、CLI 和网关默认使用的 Agent。
 */
export function AgentSurfaceDefaults({ config, options, onConfigChange }: AgentSurfaceDefaultsProps) {
  const { locale, t } = useI18n();
  const surfaces: Array<{ field: SurfaceField; label: string; description: string }> = [
    { field: "default_agent", label: "Web", description: t("Used when the web workspace has no explicit Agent selection", "网页工作台未显式选择 Agent 时采用") },
    { field: "tui_agent", label: "TUI", description: t("Used when an interactive terminal session starts", "交互式终端会话启动时采用") },
    { field: "cli_agent", label: "CLI", description: t("Used for one-shot ask, message arguments, and Shell interception", "单次 ask、消息参数和 Shell 拦截采用") },
    { field: "gateway_agent", label: t("Gateway", "网关"), description: t("Used for QQ, WeChat, and other messaging gateway sessions", "QQ / 微信等消息网关会话采用") }
  ];
  const profiles = buildVisibleAgentProfiles(config.agents, options, config.subagent?.profiles, locale);
  const choices = profiles.map((profile) => ({
    value: profile.id,
    label: profile.name || profile.id,
    description: profile.description || undefined
  }));

  const update = (field: SurfaceField, value: string) => {
    onConfigChange({ ...config, [field]: value === "default" ? null : value });
  };

  const valueOf = (field: SurfaceField) => {
    if (field === "default_agent") return config.default_agent ?? "default";
    if (field === "tui_agent") return config.tui_agent ?? "default";
    if (field === "cli_agent") return config.cli_agent ?? "default";
    return config.gateway_agent ?? "gateway";
  };

  return <SettingsPanel title={t("Default Agent by entry point", "入口默认 Agent")}>
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {surfaces.map((surface) => <SettingsField key={surface.field} label={surface.label} anchor={`agents.${surface.field}`} hint={surface.description}>
        <SkSelect value={valueOf(surface.field)} options={choices} onChange={(next) => update(surface.field, next)} />
      </SettingsField>)}
    </div>
  </SettingsPanel>;
}
