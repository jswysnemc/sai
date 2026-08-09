import { RefreshCw } from "lucide-react";
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { AppConfig } from "../../../api/contracts";
import { toDisplayError } from "../../../api/api-error";
import { useI18n } from "../../i18n/use-i18n";
import { Button } from "../../../shared/ui/button/button";
import { AgentProfileWorkspace } from "./agent-profile-workspace";
import { AgentSurfaceDefaults } from "./agent-surface-defaults";
import { fetchAgentMcpOptions, fetchAgentOptions, mergeAgentOptions } from "./agents-api";
import "./agent-settings-layout.css";
import "./agent-profile-form.css";

type AgentSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染统一 Agent 配置工作区。
 *
 * 能力选项分两段加载：本地工具与 Skills 决定首屏；MCP 动态工具
 * 可能涉及网络或子进程，在本地选项就绪后后台发现并合并，失败静默。
 * 请求竞态与缓存由 React Query 承担，不再手写 generation 计数。
 *
 * @param props 应用配置和更新回调
 * @returns Agent 设置区域
 */
export function AgentSettingsSection({ config, onConfigChange }: AgentSettingsSectionProps) {
  const { t } = useI18n();
  const local = useQuery({ queryKey: ["agent-options"], queryFn: fetchAgentOptions });
  const mcp = useQuery({
    queryKey: ["agent-options", "mcp"],
    queryFn: fetchAgentMcpOptions,
    enabled: local.isSuccess,
    retry: false
  });
  // 1. MCP 选项就绪后并入本地选项并按名称去重
  const options = useMemo(() => {
    const base = local.data ?? { tools: [], skills: [] };
    return mcp.data ? mergeAgentOptions(base, mcp.data) : base;
  }, [local.data, mcp.data]);
  const loading = local.isLoading;
  const error = local.error
    ? toDisplayError(local.error, "Failed to load Agent capabilities", "Agent 能力加载失败")
    : null;

  return (
    <section className="agent-settings-shell">
      {loading && (
        <div className="agent-settings-loading" aria-live="polite">
          <span />
          <div><strong>{t("Loading Agent capabilities", "正在读取 Agent 能力")}</strong><small>{t("Loading tools and Skills", "加载工具和 Skills 列表")}</small></div>
        </div>
      )}
      {!loading && error && (
        <div className="agent-settings-load-error">
          <div><strong>{t("Failed to load Agent capabilities", "Agent 能力加载失败")}</strong><small>{error.message}</small></div>
          <Button className="settings-secondary" onClick={() => void local.refetch()}>
            <RefreshCw size={14} />{t("Reload", "重新加载")}
          </Button>
        </div>
      )}
      {!loading && !error && (
        <>
          <AgentSurfaceDefaults config={config} options={options} onConfigChange={onConfigChange} />
          <AgentProfileWorkspace config={config} options={options} onConfigChange={onConfigChange} />
        </>
      )}
    </section>
  );
}
