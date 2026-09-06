import { DiffView } from "../chat/tool-renderers/diff-view";
import { DiffViewControls } from "../chat/tool-renderers/diff/diff-view-controls";
import { useDiffViewOptions } from "../chat/tool-renderers/diff/use-diff-view-options";
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
  const display = useDiffViewOptions("side");
  if (!source.trim()) {
    return <div className="workspace-targeted-diff-empty">{t("No diff content is available", "没有可显示的差异内容")}</div>;
  }
  return (
    <div className="workspace-targeted-diff" ref={display.ref} aria-label={t(`Diff for ${path}`, `${path} 的差异`)}>
      <header className="workspace-targeted-diff-head">
        <span title={path}>{path}</span>
        <DiffViewControls options={display} />
      </header>
      <DiffView source={source} headerPath={path} onlyPath={path} layout={display.layout} wrap={display.wrap} review />
    </div>
  );
}
