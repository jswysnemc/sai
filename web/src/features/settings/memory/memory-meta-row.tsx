import type { MemoryStats } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";

type MemoryMetaRowProps = {
  stats?: MemoryStats;
  /** 可选工作区数量，超过 1 才显示切换器 */
  workspaceCount: number;
  /** 当前选中的工作区标识 */
  selectedWorkspace?: string;
  /** 工作区下拉选项 */
  workspaceOptions: Array<{ value: string; label: string }>;
  /** 记忆文件所在目录，悬停可查看完整路径 */
  notesDir?: string;
  onWorkspaceChange: (value: string) => void;
};

/**
 * 条目列表上方的紧凑元信息行：统计数字、工作区切换与存储路径。
 *
 * 统计从三张大卡片压成一行内联数字——数量只用来判断规模，
 * 值得占据视觉空间的是条目列表本身。
 *
 * @param props 统计数据、工作区信息与回调
 * @returns 元信息行
 */
export function MemoryMetaRow({
  stats,
  workspaceCount,
  selectedWorkspace,
  workspaceOptions,
  notesDir,
  onWorkspaceChange
}: MemoryMetaRowProps) {
  const { t } = useI18n();

  return (
    <div className="memory-meta-row">
      <dl className="memory-meta-stats">
        <div className="memory-meta-stat">
          <dt>{t("Project", "项目")}</dt>
          <dd>{stats?.project_memories ?? 0}</dd>
        </div>
        <div className="memory-meta-stat">
          <dt>{t("Global", "全局")}</dt>
          <dd>{stats?.global_memories ?? 0}</dd>
        </div>
        <div className="memory-meta-stat">
          <dt>{t("Evicted", "逐出")}</dt>
          <dd>{stats?.evicted_turns ?? 0}</dd>
        </div>
      </dl>

      {workspaceCount > 1 && (
        <Select
          value={selectedWorkspace ?? workspaceOptions[0]?.value ?? ""}
          options={workspaceOptions}
          ariaLabel={t("Choose workspace", "选择工作区")}
          onChange={onWorkspaceChange}
        />
      )}

      {notesDir && (
        <code className="memory-meta-path" title={notesDir}>
          {notesDir}
        </code>
      )}
    </div>
  );
}
