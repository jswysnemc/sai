import { CheckCircle2, XCircle } from "../../../shared/ui/icons";
import type { JevProbeReport } from "../../../api/contracts/jev";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 渲染一次 Jev 连接测试的结果行。
 * @param props.report 测试结果
 * @returns 结果、实际模型、耗时与说明的单行摘要
 */
export function JevProbeResult({ report }: { report: JevProbeReport }) {
  const { t } = useI18n();
  const model = report.served_model ?? report.model;
  return (
    <div className={report.ok ? "model-endpoint-probe ok" : "model-endpoint-probe failed"}>
      {report.ok ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
      <span>{report.ok ? t(`Connected · ${model}`, `连接正常 · ${model}`) : t("Request failed", "请求失败")}</span>
      <em>{report.duration_ms} ms</em>
      {report.detail && <small title={report.detail}>{report.detail}</small>}
    </div>
  );
}
