import { CheckCircle2, Loader2, PlugZap, Wrench, XCircle } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { ProviderConfig, ProviderProbeReport } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { probeHint, stageLabel } from "./provider-probe-hints";
import "./provider-connection-test.css";

type ProviderConnectionTestProps = {
  provider: ProviderConfig;
  /** 待测模型；为空时后端取供应商默认模型 */
  model?: string;
  /** 多密钥时实际用于探测的密钥标识 */
  selectedKeyId?: string;
};

type ProviderProbeMode = "connection" | "tools";

/**
 * 构造只使用一个密钥的探测配置，避免探测请求被负载均衡分配到其他密钥。
 *
 * @param provider 当前供应商配置
 * @param selectedKeyId 当前选中的密钥标识
 * @returns 固定使用指定密钥的探测配置
 */
function providerForProbe(provider: ProviderConfig, selectedKeyId?: string): ProviderConfig {
  const keys = provider.api_keys ?? [];
  if (keys.length === 0) return { ...provider, api_key_balance: false };
  const selected = keys.find((key) => key.id === selectedKeyId) ?? keys[0];
  return {
    ...provider,
    api_keys: [selected],
    api_key_selected: selected.id,
    api_key_balance: false
  };
}

/**
 * 渲染供应商连通性测试控件。
 *
 * 测试分两个阶段执行：先验地址与凭据，再发一条最小对话验模型可用。
 * 分阶段展示是为了让失败能落到具体一环，而不是笼统的"连接失败"。
 *
 * @param props 供应商配置与待测模型
 * @returns 测试按钮与结果面板
 */
export function ProviderConnectionTest({ provider, model, selectedKeyId }: ProviderConnectionTestProps) {
  const { t } = useI18n();
  const [running, setRunning] = useState(false);
  const [runningMode, setRunningMode] = useState<ProviderProbeMode | null>(null);
  const [report, setReport] = useState<ProviderProbeReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  /**
   * 执行一次连通性探测。
   *
   * @returns 无
   */
  const runTest = async (mode: ProviderProbeMode) => {
    setRunning(true);
    setRunningMode(mode);
    setError(null);
    setReport(null);
    try {
      setReport(await api.providers.test(providerForProbe(provider, selectedKeyId), model, mode));
    } catch (reason) {
      setError(toDisplayError(reason, "Connection test failed", "连通性测试失败").message);
    } finally {
      setRunning(false);
      setRunningMode(null);
    }
  };

  const hint = report && !report.ok ? probeHint(report.error_kind, t) : "";

  return (
    <div className="provider-probe">
      <div className="provider-probe-head">
        <div className="provider-probe-actions">
          <Button className="provider-probe-run" disabled={running} onClick={() => void runTest("connection")}>
            {runningMode === "connection" ? <Loader2 size={13} className="provider-probe-spin" /> : <PlugZap size={13} />}
            {runningMode === "connection" ? t("Testing", "测试中") : t("Test connection", "普通连通性测试")}
          </Button>
          <Button className="provider-probe-run" disabled={running} onClick={() => void runTest("tools")}>
            {runningMode === "tools" ? <Loader2 size={13} className="provider-probe-spin" /> : <Wrench size={13} />}
            {runningMode === "tools" ? t("Testing tools", "工具测试中") : t("Test tools", "工具测试")}
          </Button>
        </div>
        {report && (
          <span className={report.ok ? "provider-probe-verdict ok" : "provider-probe-verdict failed"}>
            {report.ok ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
            {report.ok ? t("Connected", "连通") : t("Failed", "未连通")}
            <em>{report.total_ms} ms</em>
          </span>
        )}
      </div>

      {error && <p className="provider-probe-error">{error}</p>}

      {report && (
        <ol className="provider-probe-stages">
          {report.stages.map((stage) => (
            <li key={stage.stage} className={stage.ok ? "ok" : "failed"}>
              <span className="provider-probe-stage-name">
                {stage.ok ? <CheckCircle2 size={12} /> : <XCircle size={12} />}
                {stageLabel(stage.stage, t)}
              </span>
              <span className="provider-probe-stage-detail" title={stage.detail}>
                {stage.detail}
              </span>
              <em>{stage.duration_ms} ms</em>
            </li>
          ))}
        </ol>
      )}

      {report?.ok && report.tokens !== undefined && (
        <p className="provider-probe-note">
          {t(`Model ${report.model} replied, ${report.tokens} tokens used.`, `模型 ${report.model} 已响应，消耗 ${report.tokens} Token。`)}
        </p>
      )}

      {hint && <p className="provider-probe-note">{hint}</p>}
    </div>
  );
}
