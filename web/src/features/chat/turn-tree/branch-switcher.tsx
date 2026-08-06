import { ChevronLeft, ChevronRight, GitBranch } from "lucide-react";
import type { SessionTurnTree } from "../../../api/turn-tree-contracts";
import { useI18n } from "../../i18n/use-i18n";
import { findSiblingBranches, preferredBranchLeafId } from "./turn-tree-rows";
import "./branch-switcher.css";

type BranchSwitcherProps = {
  tree?: SessionTurnTree;
  /** 当前消息对应的轮次 */
  turnId: string;
  busy?: boolean;
  onSwitch: (turnId: string) => void;
};

/**
 * 在存在分叉的消息下方显示版本切换器。
 *
 * 同一个提问点可能走过多条路径，这里用「‹ 2/3 ›」表达当前处在第几条，
 * 左右箭头在同级分支之间切换。没有分叉时不渲染任何内容。
 *
 * @param props 树数据、当前轮次与切换回调
 * @returns 版本切换器；无分叉时返回 null
 */
export function BranchSwitcher({ tree, turnId, busy, onSwitch }: BranchSwitcherProps) {
  const { t } = useI18n();
  if (!tree) return null;
  const group = findSiblingBranches(tree, turnId);
  if (!group) return null;

  const { siblings, index } = group;
  const goTo = (target: number) => {
    const next = siblings[target];
    if (next) onSwitch(preferredBranchLeafId(next));
  };

  return (
    <div className="branch-switcher" aria-label={t("Branch versions", "分支版本")}>
      <GitBranch size={11} aria-hidden />
      <button
        type="button"
        disabled={busy || index === 0}
        onClick={() => goTo(index - 1)}
        aria-label={t("Previous version", "上一个版本")}
        title={t("Previous version", "上一个版本")}
      >
        <ChevronLeft size={12} />
      </button>
      <span className="branch-switcher-count">
        {index + 1} / {siblings.length}
      </span>
      <button
        type="button"
        disabled={busy || index + 1 >= siblings.length}
        onClick={() => goTo(index + 1)}
        aria-label={t("Next version", "下一个版本")}
        title={t("Next version", "下一个版本")}
      >
        <ChevronRight size={12} />
      </button>
    </div>
  );
}
