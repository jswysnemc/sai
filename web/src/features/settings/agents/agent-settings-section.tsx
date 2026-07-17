import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { AppConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { Button } from "../../../shared/ui/button/button";
import { AgentProfileWorkspace } from "./agent-profile-workspace";
import { AgentSurfaceDefaults } from "./agent-surface-defaults";
import { fetchAgentOptions } from "./agents-api";
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
  const [error, setError] = useState("");

  /**
   * 读取主 Agent 可用的工具与 Skills，并同步加载状态。
   *
   * @returns 加载流程完成后的 Promise
  */
  const loadOptions = useCallback(async () => {
    // 1. 重置上一次加载状态
    setLoading(true);
    setError("");
    // 2. 请求能力选项并记录可展示的错误信息
    try {
      setOptions(await fetchAgentOptions());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadOptions();
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
          <div><strong>{t("Failed to load Agent capabilities", "Agent 能力加载失败")}</strong><small>{error}</small></div>
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
