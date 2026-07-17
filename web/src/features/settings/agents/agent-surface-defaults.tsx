import type { AppConfig } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { buildVisibleAgentProfiles } from "./agent-profile-state";
import type { AgentOptions } from "./agents-types";

type AgentSurfaceDefaultsProps = {
  config: AppConfig;
  options: AgentOptions;
  onConfigChange: (config: AppConfig) => void;
};

type SurfaceField = "default_agent" | "tui_agent" | "cli_agent" | "gateway_agent";

const SURFACES: Array<{ field: SurfaceField; label: string; description: string }> = [
  {
    field: "default_agent",
    label: "Web",
    description: "网页工作台未显式选择 Agent 时采用"
  },
  {
    field: "tui_agent",
    label: "TUI",
    description: "交互式终端会话启动时采用"
  },
  {
    field: "cli_agent",
    label: "CLI",
    description: "单次 ask、消息参数和 Shell 拦截采用"
  },
  {
    field: "gateway_agent",
    label: "网关",
    description: "QQ / 微信等消息网关会话采用"
  }
];

/**
 * 配置 Web、TUI、CLI 和网关默认使用的 Agent。
 */
export function AgentSurfaceDefaults({ config, options, onConfigChange }: AgentSurfaceDefaultsProps) {
  const profiles = buildVisibleAgentProfiles(config.agents, options, config.subagent?.profiles);
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

  const nameOf = (id: string) => choices.find((choice) => choice.value === id)?.label ?? id;

  return (
    <section className="agent-surface-defaults">
      <div className="settings-section-heading">
        <div>
          <strong>入口默认 Agent</strong>
          <small>分别控制网页工作台、TUI REPL、单次 CLI 和消息网关。</small>
        </div>
      </div>
      <div className="agent-surface-grid">
        {SURFACES.map((surface) => {
          const value = valueOf(surface.field);
          return (
            <article key={surface.field} className="agent-surface-card">
              <header>
                <strong>{surface.label}</strong>
                <span>{nameOf(value)}</span>
              </header>
              <p>{surface.description}</p>
              <Select
                value={value}
                options={choices}
                onChange={(next) => update(surface.field, next)}
                ariaLabel={`${surface.label} 默认 Agent`}
              />
            </article>
          );
        })}
      </div>
    </section>
  );
}
