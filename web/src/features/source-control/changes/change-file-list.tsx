import { ChevronRight, Folder, FolderOpen } from "lucide-react";
import { useMemo, useState } from "react";
import type { GitStatusEntry, ScmConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { ChangeSectionKind } from "./change-section";
import { ChangeFileRow } from "./change-file-row";
import { buildGitChangeTreeRows } from "./change-tree";
import "./change-file-list.css";

/**
 * 单个分区默认渲染的行数上限。
 *
 * 大仓库首次初始化时未跟踪文件动辄数千，全量挂载会让整个面板卡住数秒。
 * 变更分区与提交图不同：多个分区共享外层滚动容器，做不到按滚动位置窗口化，
 * 因此改为限制初始渲染量，并在界面上明确给出被折叠的数量与展开入口。
 */
const DEFAULT_ROW_LIMIT = 200;

type ChangeFileListProps = {
  entries: GitStatusEntry[];
  viewMode: ScmConfig["default_view_mode"];
  selectedPath: string | null;
  selectedPaths: ReadonlySet<string>;
  busy: boolean;
  section: ChangeSectionKind;
  onSelect: (path: string, event: React.MouseEvent<HTMLButtonElement>) => void;
  onContextMenu: (path: string, event: React.MouseEvent<HTMLDivElement>) => void;
  onStage: (path: string) => void;
  onUnstage: (path: string) => void;
  onIgnore: (path: string) => void;
  onDiscard: (entry: GitStatusEntry) => void;
};

/**
 * 按列表或可折叠目录树渲染 Git 文件。
 *
 * @param props 文件条目、视图模式和操作回调
 * @returns 文件列表或目录树
 */
export function ChangeFileList(props: ChangeFileListProps) {
  const { t } = useI18n();
  const [collapsedPaths, setCollapsedPaths] = useState<Set<string>>(() => new Set());
  const [showAll, setShowAll] = useState(false);
  const treeRows = useMemo(
    () => buildGitChangeTreeRows(props.entries, collapsedPaths),
    [collapsedPaths, props.entries]
  );

  /**
   * 切换单个目录的展开状态。
   *
   * @param path 仓库相对目录路径
   * @returns 无返回值
   */
  const toggleDirectory = (path: string) => {
    setCollapsedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  if (props.entries.length === 0) {
    return <div className="git-clean">{t("No files", "无文件")}</div>;
  }

  const tree = props.viewMode === "tree";
  const totalRows = tree ? treeRows.length : props.entries.length;
  const limit = showAll ? totalRows : DEFAULT_ROW_LIMIT;
  const hiddenRows = Math.max(0, totalRows - limit);

  return (
    <div className={`git-file-list git-file-list-${props.viewMode}`}>
      {tree
        ? treeRows.slice(0, limit).map((row) => {
            if (row.kind === "directory") {
              const collapsed = collapsedPaths.has(row.path);
              return (
                <Button
                  key={`directory:${row.path}`}
                  className="git-tree-directory"
                  style={{ paddingInlineStart: `${0.25 + row.depth * 0.875}rem` }}
                  onClick={() => toggleDirectory(row.path)}
                  aria-expanded={!collapsed}
                  title={row.path}
                >
                  <ChevronRight size={11} className={collapsed ? "" : "open"} />
                  {collapsed ? <Folder size={13} /> : <FolderOpen size={13} />}
                  <span>{row.name}</span>
                </Button>
              );
            }
            return renderFileRow(props, row.entry, row.name, row.depth);
          })
        : props.entries.slice(0, limit).map((entry) => renderFileRow(props, entry, entry.path, 0))}
      {hiddenRows > 0 && (
        <Button className="git-file-list-more" onClick={() => setShowAll(true)}>
          {t(`Show ${hiddenRows} more`, `展开其余 ${hiddenRows} 项`)}
        </Button>
      )}
    </div>
  );
}

/**
 * 组合单个文件行需要的选择和操作回调。
 *
 * @param props 文件列表公共属性
 * @param entry 当前文件状态
 * @param displayName 当前模式显示名称
 * @param depth 树形缩进深度
 * @returns 文件行组件
 */
function renderFileRow(
  props: ChangeFileListProps,
  entry: GitStatusEntry,
  displayName: string,
  depth: number
) {
  return (
    <ChangeFileRow
      key={`${props.section}:${entry.index_status}${entry.worktree_status}:${entry.path}`}
      entry={entry}
      displayName={displayName}
      depth={depth}
      active={props.selectedPath === entry.path}
      selected={props.selectedPaths.has(entry.path)}
      busy={props.busy}
      section={props.section}
      onSelect={(event) => props.onSelect(entry.path, event)}
      onContextMenu={(event) => props.onContextMenu(entry.path, event)}
      onStage={() => props.onStage(entry.path)}
      onUnstage={() => props.onUnstage(entry.path)}
      onIgnore={() => props.onIgnore(entry.path)}
      onDiscard={() => props.onDiscard(entry)}
    />
  );
}
