import { Columns2, Rows3, WrapText } from "lucide-react";
import { Button } from "../../../../shared/ui/button/button";
import { useI18n } from "../../../i18n/use-i18n";
import type { useDiffViewOptions } from "./use-diff-view-options";
import "./diff-view-controls.css";

/**
 * 渲染统一、并排与自动换行控件，窄容器明确禁用并排模式。
 * @param props 当前显示设置及更新方法
 * @returns 项目统一按钮组成的差异显示控件
 */
export function DiffViewControls({ options }: { options: ReturnType<typeof useDiffViewOptions> }) {
  const { t } = useI18n();
  return <div className="diff-view-controls" role="group" aria-label={t("Diff display", "差异显示")}>
    <Button variant="ghost" size="small" aria-pressed={options.layout === "unified"} onClick={() => options.setLayout("unified")}
      aria-label={t("Unified view", "统一视图")} title={t("Unified view", "统一视图")}>
      <Rows3 size={14} /><span className="diff-view-control-label hidden sm:inline">{t("Unified", "统一")}</span>
    </Button>
    <Button variant="ghost" size="small" aria-pressed={options.layout === "side"} disabled={!options.sideAvailable} onClick={() => options.setLayout("side")}
      aria-label={t("Side by side view", "并排对比")}
      title={options.sideAvailable ? t("Side by side view", "并排对比") : t("Widen the panel to compare side by side", "加宽面板后可并排对比")}>
      <Columns2 size={14} /><span className="diff-view-control-label hidden sm:inline">{t("Split", "并排")}</span>
    </Button>
    <Button variant="ghost" size="icon" aria-pressed={options.wrap} onClick={() => options.setWrap(!options.wrap)}
      aria-label={t("Wrap lines", "自动换行")} title={t("Wrap lines", "自动换行")}>
      <WrapText size={14} />
    </Button>
  </div>;
}
