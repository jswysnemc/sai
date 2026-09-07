import { Folder, Folders } from "lucide-react";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import "./session-scope-control.css";

export type SessionScope = "current" | "all";
export const SESSION_SCOPE_KEY = "sai.sidebar.session-scope";

/**
 * 【会话侧栏】【范围选择】通过紧凑下拉选择当前工作区或全部工作区的会话。
 * @param props 当前范围和切换回调
 * @returns 紧凑范围控件
 */
export function SessionScopeControl({ value, onChange }: { value: SessionScope; onChange: (value: SessionScope) => void }) {
  const { t } = useI18n();
  const label = value === "current" ? t("Current workspace", "当前工作区") : t("All workspaces", "全部工作区");
  return <div className="session-scope-control" title={label}>
    <Select value={value} onChange={onChange} ariaLabel={t(`Session workspace scope: ${label}`, `会话工作区范围：${label}`)} menuPreferredWidth={180} menuMinimumWidth={160} menuAlign="right" options={[
      { value: "current", label: t("Current workspace", "当前工作区"), icon: <Folder size={13} /> },
      { value: "all", label: t("All workspaces", "全部工作区"), icon: <Folders size={13} /> }
    ]} />
  </div>;
}
