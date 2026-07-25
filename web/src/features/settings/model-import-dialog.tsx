import { Check, Search } from "lucide-react";
import { useDeferredValue, useEffect, useState } from "react";
import { Modal } from "../../shared/ui/dialog/modal";
import { ModelIcon } from "../../shared/ui/model-icon";
import { useI18n } from "../i18n/use-i18n";

export type ImportModelMetadata = {
  provider?: string | null;
  context_chars?: number | null;
  max_output_tokens?: number | null;
  tags?: string[];
};

type ModelImportDialogProps = {
  open: boolean;
  models: string[];
  existingModels: string[];
  metadata?: Record<string, ImportModelMetadata>;
  onClose: () => void;
  onImport: (models: string[]) => void;
};

/**
 * 渲染远端模型搜索、勾选和选择性导入弹层。
 *
 * @param props 远端模型、现有模型、元数据和操作回调
 * @returns 模型导入弹层
 */
export function ModelImportDialog({
  open,
  models,
  existingModels,
  metadata = {},
  onClose,
  onImport
}: ModelImportDialogProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const deferredQuery = useDeferredValue(query.trim().toLowerCase());
  const filtered = models.filter((model) => !deferredQuery || model.toLowerCase().includes(deferredQuery));

  useEffect(() => {
    if (!open) {
      setQuery("");
      setSelected([]);
    }
  }, [open]);

  /**
   * 切换远端模型选择状态。
   *
   * @param model 模型标识
   */
  const toggle = (model: string) => {
    if (existingModels.includes(model)) return;
    setSelected((current) => current.includes(model) ? current.filter((item) => item !== model) : [...current, model]);
  };

  return (
    <Modal
      open={open}
      title={t("Import remote models", "导入远端模型")}
      description={t("Fetched results are not written directly to configuration. Only selected models are imported.", "获取结果不会直接写入配置，只导入本次勾选的模型。")}
      size="large"
      onClose={onClose}
      footer={<><span className="model-import-count">{t(`${selected.length} selected`, `已选择 ${selected.length} 个`)}</span><button type="button" className="ui-button secondary" onClick={onClose}>{t("Cancel", "取消")}</button><button type="button" className="ui-button primary" disabled={selected.length === 0} onClick={() => onImport(selected)}>{t("Import models", "导入模型")}</button></>}
    >
      <div className="model-import-dialog">
        <label className="model-import-search"><Search size={14} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Search model IDs", "搜索模型 ID")} autoFocus /></label>
        <div className="model-import-list">
          {filtered.map((model) => {
            const existing = existingModels.includes(model);
            const active = selected.includes(model);
            const item = metadata[model] ?? {};
            const summary = formatModelMetadataSummary(item, t);
            return (
              <button
                type="button"
                className={active ? "active" : ""}
                disabled={existing}
                key={model}
                onClick={() => toggle(model)}
              >
                <ModelIcon model={model} provider={item.provider} size={16} />
                <span>
                  <strong>{model}</strong>
                  <small>{existing ? t("Already added", "已经添加") : summary || t("Available to import", "可导入")}</small>
                </span>
                {(active || existing) && <Check size={14} />}
              </button>
            );
          })}
          {filtered.length === 0 && <div className="model-import-empty">{t("No matching models", "没有匹配的模型")}</div>}
        </div>
      </div>
    </Modal>
  );
}

/**
 * 将上下文、输出上限和标签整理为导入列表副标题。
 *
 * @param metadata 模型元数据
 * @param t 双语翻译函数
 * @returns 摘要文本；无元数据时返回空字符串
 */
function formatModelMetadataSummary(
  metadata: ImportModelMetadata,
  t: (en: string, zh: string) => string
): string {
  const parts: string[] = [];
  if (metadata.context_chars) {
    parts.push(`${t("Context", "上下文")} ${formatTokenCount(metadata.context_chars)}`);
  }
  if (metadata.max_output_tokens) {
    parts.push(`${t("Max output", "最大输出")} ${formatTokenCount(metadata.max_output_tokens)}`);
  }
  if (metadata.tags?.length) {
    parts.push(metadata.tags.join(", "));
  }
  return parts.join(" · ");
}

/**
 * 将 token 数量格式化为紧凑展示。
 *
 * @param value token 数量
 * @returns 如 128k / 1m 的文本
 */
function formatTokenCount(value: number): string {
  if (value >= 1_000_000 && value % 1_000_000 === 0) {
    return `${value / 1_000_000}m`;
  }
  if (value >= 1_000 && value % 1_000 === 0) {
    return `${value / 1_000}k`;
  }
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1).replace(/\.0$/, "")}m`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1).replace(/\.0$/, "")}k`;
  }
  return String(value);
}
