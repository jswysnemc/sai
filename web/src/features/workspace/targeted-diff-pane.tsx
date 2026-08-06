import { Columns2, Rows3 } from "lucide-react";
import { useState } from "react";
import { Button } from "../../shared/ui/button/button";
import { DiffView, type DiffLayout } from "../chat/tool-renderers/diff-view";
import { useI18n } from "../i18n/use-i18n";

type TargetedDiffPaneProps = {
  path: string;
  source: string;
};

/**
 * 渲染由文件改动或比较动作被动打开的具体 Diff。
 *
 * 参数:
 * - `props`: 文件路径和补丁文本
 *
 * 返回:
 * - 可在统一视图与并排视图之间切换的 Diff 面板
 */
export function TargetedDiffPane({ path, source }: TargetedDiffPaneProps) {
  const { t } = useI18n();
  const [layout, setLayout] = useState<DiffLayout>("side");
  if (!source.trim()) {
    return <div className="workspace-targeted-diff-empty">{t("No diff content is available", "没有可显示的差异内容")}</div>;
  }
  return (
    <div className="workspace-targeted-diff" aria-label={t(`Diff for ${path}`, `${path} 的差异`)}>
      <header className="workspace-targeted-diff-head">
        <span>{path}</span>
        <span className="workspace-targeted-diff-layout-toggle" role="group" aria-label={t("Diff layout", "差异布局")}>
          <Button
            className={layout === "unified" ? "is-active" : ""}
            onClick={() => setLayout("unified")}
            title={t("Unified view", "统一视图")}
            aria-label={t("Unified view", "统一视图")}
          >
            <Rows3 size={13} />
          </Button>
          <Button
            className={layout === "side" ? "is-active" : ""}
            onClick={() => setLayout("side")}
            title={t("Side by side view", "并排对比")}
            aria-label={t("Side by side view", "并排对比")}
          >
            <Columns2 size={13} />
          </Button>
        </span>
      </header>
      <DiffView source={source} headerPath={path} onlyPath={path} layout={layout} />
    </div>
  );
}
