import type { ProviderConfig } from "../../../api/contracts";
import { ModelMetadataEditor } from "../model-metadata-editor";

type ProviderModelsTabProps = {
  provider: ProviderConfig;
  onChange: (patch: Partial<ProviderConfig>) => void;
};

/**
 * 供应商编辑器的模型页签：模型目录与逐模型元数据。
 *
 * 目录标题和编辑逻辑都由 ModelMetadataEditor 承担。
 *
 * @param props 供应商状态与更新回调
 * @returns 模型页签内容
 */
export function ProviderModelsTab({ provider, onChange }: ProviderModelsTabProps) {
  return (
    <section className="model-catalog-section">
      <ModelMetadataEditor
        // 切换供应商时重建：选中模型、新模型草稿和上下文单位都是内部状态，
        // 复用实例会把上一个供应商的选择带过来
        key={provider.id}
        provider={provider}
        onChange={onChange}
      />
    </section>
  );
}
