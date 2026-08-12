import { useI18n } from "../../i18n/use-i18n";
import type { ModelSwitchMarker } from "../model-switch-divider";

/**
 * 渲染模型切换分割线：弱化横线加居中小字，标注前后模型。
 *
 * @param props marker 为相邻轮次派生出的模型切换标记
 * @returns 模型切换分割线
 */
export function ModelSwitchDivider({ marker }: { marker: ModelSwitchMarker }) {
  const { t } = useI18n();
  const label = t(
    `Model switched: ${marker.from} → ${marker.to}`,
    `模型已切换：${marker.from} → ${marker.to}`
  );
  return (
    <div className="model-switch-divider" role="separator" aria-label={label}>
      <span className="model-switch-divider-label" title={label}>{label}</span>
    </div>
  );
}
