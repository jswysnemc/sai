import type { SessionDataSummary } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { DataTable, StatusBadge } from "../kit";
import { formatSessionBytes, formatSessionDate } from "./session-data-format";
import { isIdleSession, sessionKey } from "./session-data-selection";

type Props = { rows: SessionDataSummary[]; selected: Set<string>; busy: boolean; onSelect: (keys: Set<string>) => void; onDetails: (session: SessionDataSummary) => void; onClear: (session: SessionDataSummary) => void; onDelete: (session: SessionDataSummary) => void };

/**
 * 【会话数据】【列表】展示可排序摘要，运行中与未知状态会话禁止清理。
 * @param props 会话、选择、操作状态与回调
 * @returns 紧凑数据表
 */
export function SessionDataTable({ rows, selected, busy, onSelect, onDetails, onClear, onDelete }: Props) {
  const { t, locale } = useI18n();
  const selectable = new Set(rows.filter(isIdleSession).map(sessionKey));
  return <DataTable label={t("Session data", "会话数据")} rows={rows} rowKey={sessionKey} selection={{ selected, onChange: onSelect, isSelectable: (key) => !busy && selectable.has(key) }} initialSort={{ id: "updated", direction: "desc" }} columns={[
    { id: "workspace", header: t("Workspace", "工作区"), sortValue: (row) => row.workspace_name, render: (row) => <span title={row.workspace_path}>{row.workspace_name}</span> },
    { id: "title", header: t("Session", "会话"), sortValue: (row) => row.title, render: (row) => <div className="sk-cell-main"><Button variant="ghost" className="whitespace-normal text-left" onClick={() => onDetails(row)}>{row.title}</Button>{row.busy === true ? <StatusBadge tone="info">{t("Running", "运行中")}</StatusBadge> : row.busy === undefined ? <small>{t("Status unavailable", "状态未知")}</small> : row.active && <small>{t("Current", "当前会话")}</small>}</div> },
    { id: "updated", header: t("Modified", "修改时间"), sortValue: (row) => row.updated_at, render: (row) => formatSessionDate(row.updated_at, locale) },
    { id: "turns", header: t("Turns", "轮次"), numeric: true, sortValue: (row) => row.turn_count ?? -1, render: (row) => row.turn_count ?? "—" },
    { id: "size", header: t("Size", "大小"), numeric: true, sortValue: (row) => row.total_bytes, render: (row) => formatSessionBytes(row.total_bytes) },
    { id: "actions", header: t("Actions", "操作"), render: (row) => <div className="flex gap-1"><Button variant="secondary" disabled={busy || !isIdleSession(row)} onClick={() => onClear(row)}>{t("Clear", "清空")}</Button><Button variant="ghost-danger" disabled={busy || !isIdleSession(row)} onClick={() => onDelete(row)}>{t("Delete", "删除")}</Button></div> }
  ]} />;
}
