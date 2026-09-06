import { GitBranch } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import "./turn-tree-panel.css";

/**
 * 在会话标题栏提供固定分支入口。
 * @param props 展开状态、分叉数量和切换回调
 * @returns 分支导航按钮
 */
export function TurnTreeNavigation({ open, count, onToggle }: { open: boolean; count: number; onToggle: () => void }) {
  const { t } = useI18n();
  return <Button variant="ghost" size="small" className="turn-tree-navigation" onClick={onToggle} aria-expanded={open} aria-label={t("Show session branches", "查看会话分支")} title={t("Session branches", "会话分支")}>
    <GitBranch size={14} /><span className="hidden xl:inline">{t("Branches", "会话分支")}</span>{count > 0 && <small>{count}</small>}
  </Button>;
}
