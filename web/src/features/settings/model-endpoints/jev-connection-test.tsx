import { CheckCircle2, Loader2, PlugZap, XCircle } from "../../../shared/ui/icons";
import { useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import type { JevProbeReport } from "../../../api/contracts/jev";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { endpointForProbe } from "./endpoint-for-probe";
import "../model/provider-connection-test.css";

type JevConnectionTestProps = {
  endpoint: ModelEndpointConfig;
  keys: ModelEndpointApiKey[];
  selectedKey?: string;
  secretSentinel: string;
};

/**
 * 用当前草稿测试 Jev 模型是否连通。
 *
 * 布局与普通供应商的连通性测试一致，只保留模型连通这一项：
 * Jev 不走聊天补全，也不做工具调用探测。
 *
 * @param props 接入草稿、密钥列表与当前选中的测试密钥
 * @returns 测试按钮与结果
 */
export function JevConnectionTest({ endpoint, keys, selectedKey, secretSentinel }: JevConnectionTestProps) {
  const { t } = useI18n();
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<JevProbeReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const canTest = endpoint.endpoint.trim().length > 0;

  /**
   * 用选中的密钥发送一次最小 Noul 请求。
   *
   * @returns 无
   */
  const runTest = async () => {
    setRunning(true);
    setError(null);
    setReport(null);
    try {
      setReport(await api.jev.test(endpointForProbe(endpoint, keys, selectedKey, secretSentinel)));
    } catch (cause) {
      setError(toDisplayError(cause, "Jev connection test failed", "Jev 连接测试失败").message);
    } finally {
      setRunning(false);
    }
  };

  const model = report ? (report.served_model ?? report.model) : "";

  return (
    <div className="provider-probe">
      <div className="provider-probe-head">
        <div className="provider-probe-actions">
          <Button className="provider-probe-run" disabled={running || !canTest} onClick={() => void runTest()}>
            {running ? <Loader2 size={14} className="provider-probe-spin" /> : <PlugZap size={14} />}
            {running ? t("Testing", "测试中") : t("Test connection", "测试连接")}
          </Button>
        </div>
        {report && (
          <span className={report.ok ? "provider-probe-verdict ok" : "provider-probe-verdict failed"}>
            {report.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
            {report.ok ? t("Connected", "连通") : t("Failed", "未连通")}
            <em>{report.duration_ms} ms</em>
          </span>
        )}
      </div>
      {error && <p className="provider-probe-error">{error}</p>}
      {report && (
        <ol className="provider-probe-stages">
          <li className={report.ok ? "ok" : "failed"}>
            <span className="provider-probe-stage-name">
              {report.ok ? <CheckCircle2 size={12} /> : <XCircle size={12} />}
              {t("Jev model", "Jev 模型")}
            </span>
            <span className="provider-probe-stage-detail" title={report.detail}>
              {report.ok ? model : report.detail}
            </span>
            <em>{report.duration_ms} ms</em>
          </li>
        </ol>
      )}
      {report?.ok && (
        <p className="provider-probe-note">
          {t(`Model ${model} answered a probe question.`, `模型 ${model} 已回答探测问题。`)}
          {report.detail ? ` ${report.detail}` : ""}
        </p>
      )}
      <p className="provider-probe-note">
        {t("Tests the unsaved draft above, not the last saved connection.", "测试的是上方未保存的草稿，不是上次已保存的接入。")}
      </p>
    </div>
  );
}
