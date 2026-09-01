import type { ProviderConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
import { ModelMetadataEditor } from "../model-metadata-editor";

type ProviderModelsTabProps = {
  provider: ProviderConfig;
  onChange: (patch: Partial<ProviderConfig>) => void;
};

/**
 * 供应商编辑器的模型页签：模型目录与逐模型元数据。
 *
 * 只包一层分组头，编辑逻辑全部由 ModelMetadataEditor 承担。
 *
 * @param props 供应商状态与更新回调
 * @returns 模型页签内容
 */
export function ProviderModelsTab({ provider, onChange }: ProviderModelsTabProps) {
  const { t } = useI18n();
  const count = provider.models?.length ?? 0;

  return (
    <SettingsGroup
      title={t("Model catalog", "模型目录")}
      description={count > 0
        ? t(`${count} models available on this provider.`, `该供应商已配置 ${count} 个模型。`)
        : t("No models yet. Use Import models to fetch the remote list.", "还没有模型。用「导入模型」拉取远端列表。")}
    >
      <ModelMetadataEditor
        // 切换供应商时重建：选中模型、新模型草稿和上下文单位都是内部状态，
        // 复用实例会把上一个供应商的选择带过来
        key={provider.id}
        provider={provider}
        onChange={onChange}
      />
    </SettingsGroup>
  );
}
