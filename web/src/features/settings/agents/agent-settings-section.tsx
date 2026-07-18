import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { AppConfig } from "../../../api/contracts";
import { toDisplayError } from "../../../api/api-error";
import { useI18n } from "../../i18n/use-i18n";
import { Button } from "../../../shared/ui/button/button";
import { AgentProfileWorkspace } from "./agent-profile-workspace";
import { AgentSurfaceDefaults } from "./agent-surface-defaults";
import { fetchAgentMcpOptions, fetchAgentOptions, mergeAgentOptions } from "./agents-api";
import type { AgentOptions } from "./agents-types";
import "./agent-settings-layout.css";
import "./agent-profile-form.css";

type AgentSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染统一 Agent 配置工作区。
 *
 * @param props 应用配置和更新回调
 * @returns Agent 设置区域
 */
export function AgentSettingsSection({ config, onConfigChange }: AgentSettingsSectionProps) {
  const { t } = useI18n();
  const [options, setOptions] = useState<AgentOptions>({ tools: [], skills: [] });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const loadGeneration = useRef(0);

  /**
   * 读取主 Agent 可用的工具与 Skills，并同步加载状态。
   *
   * @returns 加载流程完成后的 Promise
  */
  const loadOptions = useCallback(async () => {
    const generation = loadGeneration.current + 1;
    loadGeneration.current = generation;
    // 1. 重置上一次加载状态
    setLoading(true);
    setError(null);
    // 2. 先读取本地选项并立即解除首屏加载状态
    try {
      const local = await fetchAgentOptions();
      if (loadGeneration.current !== generation) return;
      setOptions(local);
      setLoading(false);
      // 3. MCP 发现可能涉及网络或子进程，在后台完成后再合并
      void fetchAgentMcpOptions()
        .then((mcp) => {
          if (loadGeneration.current === generation) {
            setOptions((current) => mergeAgentOptions(current, mcp));
          }
        })
        .catch(() => undefined);
    } catch (reason) {
      if (loadGeneration.current !== generation) return;
      setError(toDisplayError(reason, "Failed to load Agent capabilities", "Agent 能力加载失败"));
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadOptions();
    return () => {
      loadGeneration.current += 1;
    };
  }, [loadOptions]);

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
          <Button className="settings-secondary" onClick={() => void loadOptions()}>
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
