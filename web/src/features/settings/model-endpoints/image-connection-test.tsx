import { CheckCircle2, Loader2, PlugZap, XCircle } from "../../../shared/ui/icons";
import { useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { ImageEndpointProbeReport, ModelEndpointApiKey, ModelEndpointConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { endpointForProbe } from "./endpoint-for-probe";
import "../model/provider-connection-test.css";

type ImageConnectionTestProps = {
  endpoint: ModelEndpointConfig;
  keys: ModelEndpointApiKey[];
  selectedKey?: string;
  secretSentinel: string;
};

/**
 * 用当前草稿测试生图模型是否连通。
 *
 * 按钮和结果区与普通供应商、Jev 接入相同。探测本身发送一次小尺寸真实生图请求。
 *
 * @param props 接入草稿、密钥列表与当前选中的测试密钥
 * @returns 测试按钮与结果
 */
export function ImageConnectionTest({ endpoint, keys, selectedKey, secretSentinel }: ImageConnectionTestProps) {
  const { t } = useI18n();
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<ImageEndpointProbeReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const canTest = endpoint.endpoint.trim().length > 0 && endpoint.model.trim().length > 0;

  /**
   * 用选中的密钥发送一次最小生图请求。
   *
   * @returns 无
   */
  const runTest = async () => {
    setRunning(true);
    setError(null);
    setReport(null);
    try {
      setReport(await api.imageModels.test(endpointForProbe(endpoint, keys, selectedKey, secretSentinel)));
    } catch (cause) {
      setError(toDisplayError(cause, "Image endpoint test failed", "生图端点测试失败").message);
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="provider-probe">
      <div className="provider-probe-head">
        <div className="provider-probe-actions">
          <Button className="provider-probe-run" disabled={running || !canTest} onClick={() => void runTest()}>
            {running ? <Loader2 size={14} className="provider-probe-spin" /> : <PlugZap size={14} />}
            {running ? t("Testing", "测试中") : t("Test image generation", "测试生图")}
          </Button>
        </div>
        {report && (
          <span className={report.ok ? "provider-probe-verdict ok" : "provider-probe-verdict failed"}>
            {report.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
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
                {t("Image model", "生图模型")}
              </span>
              <span className="provider-probe-stage-detail" title={stage.detail}>{stage.detail}</span>
              <em>{stage.duration_ms} ms</em>
            </li>
          ))}
        </ol>
      )}
      {report?.ok && (
        <p className="provider-probe-note">
          {t(`Model ${report.model} returned an image.`, `模型 ${report.model} 已返回图片。`)}
        </p>
      )}
      <p className="provider-probe-note">
        {t("Tests the unsaved draft above, not the last saved connection.", "测试的是上方未保存的草稿，不是上次已保存的接入。")}
      </p>
    </div>
  );
}
