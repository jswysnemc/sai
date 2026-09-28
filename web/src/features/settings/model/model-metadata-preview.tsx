import { Copy } from "../../../shared/ui/icons";
import type { ModelMetadata } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Toast, useToast } from "../../../shared/ui/notify/notify";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsPanel } from "../kit";

/**
 * 【供应商设置】【元数据预览】只展示模型标识与能力配置，便于复制到其他供应商。
 * @param props 模型标识和当前元数据
 * @returns 可复制的 JSON 预览，不包含连接凭据
 */
export function ModelMetadataPreview({ model, metadata }: { model: string; metadata: ModelMetadata }) {
  const { t } = useI18n();
  const { notice, showToast, dismissToast } = useToast();
  const text = JSON.stringify({ model, metadata }, null, 2);
  /** 复制当前模型元数据；无参数，完成后展示操作结果 */
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      showToast(t("Model metadata copied", "已复制模型元数据"));
    } catch { showToast(t("Clipboard unavailable", "无法访问剪贴板"), "error"); }
  };
  return <SettingsPanel title={t("Metadata preview", "元数据预览")} collapsible defaultOpen={false}>
    <Button size="small" onClick={() => void copy()}><Copy size={14} />{t("Copy metadata", "复制元数据")}</Button>
    <pre className="m-0 mt-2 max-h-52 overflow-auto whitespace-pre-wrap break-words rounded border border-[var(--line)] bg-[var(--paper-deep)] p-2 text-xs">{text}</pre>
    <Toast notice={notice} onDismiss={dismissToast} />
  </SettingsPanel>;
}
