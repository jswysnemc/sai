import { GitBranch, Maximize2, X } from "lucide-react";
import type { SessionTurnTree } from "../../../api/turn-tree-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { flattenTurnTree } from "./turn-tree-rows";
import "./turn-tree-panel.css";

type TurnTreePanelProps = {
  tree: SessionTurnTree;
  /** 是否正在切换，用于禁用重复点击 */
  busy?: boolean;
  onSelect: (turnId: string) => void;
  onClose: () => void;
  /** 打开整树总览 */
  onOpenOverview?: () => void;
};

/**
 * 以缩进树的形式展示会话的全部分支。
 *
 * 当前所在轮次高亮，活动路径上的祖先用较浅的强调色，其余分支保持中性。
 * 点击任意节点即切换到该轮次。
 *
 * @param props 树数据、忙碌状态与回调
 * @returns 分支树面板
 */
export function TurnTreePanel({ tree, busy, onSelect, onClose, onOpenOverview }: TurnTreePanelProps) {
  const { t } = useI18n();
  const rows = flattenTurnTree(tree);

  return (
    <aside className="turn-tree-panel" aria-label={t("Session branches", "会话分支")}>
      <header className="turn-tree-head">
        <GitBranch size={14} aria-hidden />
        <strong>{t("Branches", "会话分支")}</strong>
        <small>
          {t(
            `${tree.total_turns} turns · ${tree.branch_points} branch points`,
            `${tree.total_turns} 轮 · ${tree.branch_points} 处分叉`
          )}
        </small>
        {onOpenOverview && (
          <Button
            className="turn-tree-close"
            onClick={onOpenOverview}
            aria-label={t("Open branch overview", "打开分支总览")}
            title={t("Open branch overview", "打开分支总览")}
          >
            <Maximize2 size={13} />
          </Button>
        )}
        <Button
          className="turn-tree-close"
          onClick={onClose}
          aria-label={t("Close branches", "关闭分支面板")}
          title={t("Close branches", "关闭分支面板")}
        >
          <X size={13} />
        </Button>
      </header>
      <div className="turn-tree-body">
        {rows.length === 0 && (
          <p className="turn-tree-empty">{t("No turns yet.", "还没有任何对话轮次。")}</p>
        )}
        {rows.map((row) => (
          <button
            key={row.node.turn_id}
            type="button"
            className={[
              "turn-tree-row",
              row.isActive ? "is-active" : "",
              row.onActivePath ? "on-path" : ""
            ]
              .filter(Boolean)
              .join(" ")}
            disabled={busy || row.isActive}
            onClick={() => onSelect(row.node.turn_id)}
            title={row.node.user_summary}
          >
            <span className="turn-tree-gutter" aria-hidden>
              {row.ancestorBars.map((hasSibling, index) => (
                <i key={index} className={hasSibling ? "bar" : "gap"} />
              ))}
              {row.depth > 0 && <i className={row.isLast ? "corner" : "tee"} />}
            </span>
            <span className="turn-tree-dot" aria-hidden />
            <span className="turn-tree-text">{row.node.user_summary || t("(empty)", "（空）")}</span>
          </button>
        ))}
      </div>
    </aside>
  );
}
