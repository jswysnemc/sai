import type { SessionDataSummary } from "../../../api/contracts";
import { DataTable } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { formatSessionBytes } from "./session-data-format";

/**
 * 【会话数据】【详情】展示状态指标与顶层数据文件。
 * @param props 当前会话摘要
 * @returns 文件明细
 */
export function SessionDataDetails({ session }: { session: SessionDataSummary }) {
  const { t } = useI18n();
  const metrics = [[t("Branches", "分叉"), session.branch_points], [t("Loaded tools", "已加载工具"), session.loaded_tool_count], [t("Todos", "待办"), session.todo_count], [t("Goal", "目标"), session.has_goal == null ? "—" : session.has_goal ? t("Present", "存在") : t("None", "无")]];
  return <div className="grid gap-3">
    <dl className="flex flex-wrap gap-4 text-xs">{metrics.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value ?? "—"}</dd></div>)}</dl>
    {session.state_error && <div className="settings-inline-error">{session.state_error}</div>}
    <DataTable label={t("Session files", "会话文件")} rows={session.items} rowKey={(item) => item.name} columns={[
      { id: "name", header: t("Data item", "数据项"), render: (item) => item.name },
      { id: "type", header: t("Type", "类型"), render: (item) => item.kind === "directory" ? t("Directory", "目录") : item.kind === "file" ? t("File", "文件") : t("Other", "其他") },
      { id: "files", header: t("Files", "文件"), numeric: true, render: (item) => item.file_count },
      { id: "size", header: t("Size", "大小"), numeric: true, sortValue: (item) => item.bytes, render: (item) => formatSessionBytes(item.bytes) }
    ]} />
  </div>;
}
