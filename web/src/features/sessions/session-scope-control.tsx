import { Folder, Folders } from "lucide-react";
import { SegmentedControl } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";
import "./session-scope-control.css";

export type SessionScope = "current" | "all";
export const SESSION_SCOPE_KEY = "sai.sidebar.session-scope";

/**
 * 选择当前工作区或全部工作区的会话。
 * @param props 当前范围和切换回调
 * @returns 紧凑范围控件
 */
export function SessionScopeControl({ value, onChange }: { value: SessionScope; onChange: (value: SessionScope) => void }) {
  const { t } = useI18n();
  return <SegmentedControl className="session-scope-control" value={value} onChange={onChange} ariaLabel={t("Session workspace scope", "会话工作区范围")} options={[
    { value: "current", label: t("Current", "当前工作区"), icon: <Folder size={12} /> },
    { value: "all", label: t("All", "全部工作区"), icon: <Folders size={12} /> }
  ]} />;
}
