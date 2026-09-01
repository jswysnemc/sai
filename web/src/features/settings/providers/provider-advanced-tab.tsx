import type { ProviderConfig } from "../../../api/contracts";
import { JsonCodeEditor } from "../../../shared/ui/code-editor/json-code-editor";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
import { KeyValueEditor } from "../key-value-editor";
import { isClaudeClientStyle, userAgentPlaceholder } from "./provider-options";

type ProviderAdvancedTabProps = {
  provider: ProviderConfig;
  onPatch: (patch: Partial<ProviderConfig>) => void;
};

/**
 * 供应商编辑器的高级页签：客户端模拟、User-Agent 与自定义请求载荷。
 *
 * 客户端模拟是这一页的主开关，决定了 UA 缺省值与可用的附加开关，
 * 因此独占第一组；请求头与 body 属于手工覆盖项，归入第二组。
 *
 * @param props 供应商状态与更新回调
 * @returns 高级页签内容
 */
export function ProviderAdvancedTab({ provider, onPatch }: ProviderAdvancedTabProps) {
  const { t } = useI18n();
  const claudeSimulation = isClaudeClientStyle(provider.client_style);

  return (
    <>
      <SettingsGroup
        title={t("Client identity", "客户端标识")}
        description={t(
          "Pretend to be a specific CLI client; some proxies only serve recognized clients.",
          "模拟特定 CLI 客户端；部分代理只服务可识别的客户端。"
        )}
      >
        <div className="settings-form-grid">
          <div className="settings-field">
            <span>{t("Client style", "客户端模拟")}</span>
            <Select
              value={provider.client_style ?? "auto"}
              options={[
                { value: "auto", label: t("Auto", "自动") },
                { value: "default", label: t("Default", "默认") },
                { value: "codex", label: "Codex CLI" },
                { value: "claude", label: "Claude Code" },
              ]}
              onChange={(value) => onPatch({ client_style: value })}
              ariaLabel={t("Client style", "客户端模拟")}
            />
            <small>{t("Codex forces Responses body and codex_cli_rs headers. Claude forces Anthropic Messages with Claude Code headers (beta, x-app, session). Use for 1M-context Claude proxies.", "Codex 强制 Responses 与 codex_cli_rs 头。Claude 强制 Anthropic Messages 与 Claude Code 头（beta、x-app、session）。适用于 1M 上下文 Claude 代理。")}</small>
          </div>
          {claudeSimulation && (
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Claude 1M context", "Claude 启用 1M 上下文")}</strong>
                <small>{t(
                  "Attach context-1m-2025-08-07 in anthropic-beta. Enabled by default.",
                  "在 anthropic-beta 中附加 context-1m-2025-08-07，默认启用。"
                )}</small>
              </span>
              <input
                type="checkbox"
                checked={provider.claude_1m_context !== false}
                onChange={(event) => onPatch({
                  claude_1m_context: event.target.checked
                })}
              />
            </label>
          )}
          <label className="settings-field">
            <span>User-Agent</span>
            <input
              value={provider.user_agent ?? ""}
              onChange={(event) => onPatch({ user_agent: event.target.value })}
              spellCheck={false}
              placeholder={userAgentPlaceholder(provider)}
            />
            <small>{t("Empty uses Codex/Claude CLI UA when Client style matches, otherwise sai/0.1. Overrides User-Agent in extra headers.", "留空时：客户端模拟为 Codex/Claude 则用对应 CLI UA，否则 sai/0.1。优先于自定义请求头中的 User-Agent。")}</small>
          </label>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Custom request payload", "自定义请求载荷")}
        description={t(
          "Merged into every model request; explicit fields take precedence.",
          "合并到每次模型请求；显式配置字段优先。"
        )}
      >
        <div className="settings-form-grid">
          <div className="settings-field full">
            <span>{t("Extra headers", "自定义请求头")}</span>
            <KeyValueEditor
              value={provider.extra_headers ?? {}}
              onChange={(extra_headers) => onPatch({ extra_headers })}
            />
            <small>{t("Merged into each model request; Authorization is not overridden", "合并到每次模型请求，不覆盖 Authorization")}</small>
          </div>
          <div className="settings-json-field full">
            <div>
              <span>{t("Custom body JSON", "自定义 body JSON")}</span>
              <small>{t("The object is merged into each model request; explicit fields take precedence", "对象会合并到每次模型请求，显式配置字段优先")}</small>
            </div>
            <JsonCodeEditor
              value={provider.extra_body || "{}"}
              onChange={(value) => onPatch({ extra_body: value === "{}" ? "" : value })}
              height={220}
              ariaLabel={t("Provider custom body JSON", "供应商自定义 body JSON")}
            />
          </div>
        </div>
      </SettingsGroup>
    </>
  );
}
