import { Check, ChevronDown, Ellipsis, GitBranch, Search } from "lucide-react";
import { useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { TextInput } from "../../../shared/ui/form/text-input";
import { updateDiffViewPreferences } from "../../chat/tool-renderers/diff/use-diff-view-options";
import { useI18n } from "../../i18n/use-i18n";
import { executeGitCommand } from "../commands/git-command-registry";
import type { RunGitOperation } from "../types";
import "./changes-review-bar.css";

export type ChangeListScope = "uncommitted" | "staged" | "unstaged";

type ChangesReviewBarProps = {
  scope: ChangeListScope;
  added: number;
  removed: number;
  branch: string;
  busy: boolean;
  query: string;
  finding: boolean;
  runOperation: RunGitOperation;
  onScopeChange: (scope: ChangeListScope) => void;
  onQueryChange: (query: string) => void;
  onFindingChange: (finding: boolean) => void;
  onExpandAll: () => void;
  onRefresh: () => void;
};

/**
 * 渲染变更列表顶部：范围、增删统计、分支、更多操作和推送。
 *
 * @param props 当前筛选范围与操作回调
 * @returns 变更列表工具条
 */
export function ChangesReviewBar(props: ChangesReviewBarProps) {
  const { t } = useI18n();
  const [scopeOpen, setScopeOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const scopes: { id: ChangeListScope; en: string; zh: string }[] = [
    { id: "uncommitted", en: "Uncommitted", zh: "未提交" },
    { id: "staged", en: "Staged", zh: "已暂存" },
    { id: "unstaged", en: "Unstaged", zh: "未暂存" }
  ];
  const current = scopes.find((item) => item.id === props.scope) ?? scopes[0];

  return (
    <div className="git-changes-review-bar">
      <div className="git-changes-review-scope">
        <button type="button" className="git-changes-review-scope-trigger" aria-expanded={scopeOpen} onClick={() => setScopeOpen((open) => !open)}>
          <ChevronDown size={12} />
          <span>{t(current.en, current.zh)}</span>
          <span className="git-changes-review-stats"><b>+{props.added}</b><i>-{props.removed}</i></span>
        </button>
        {scopeOpen && (
          <div className="git-changes-review-menu" role="menu">
            {scopes.map((item) => (
              <button key={item.id} type="button" role="menuitem" onClick={() => { props.onScopeChange(item.id); setScopeOpen(false); }}>
                <span>{t(item.en, item.zh)}</span>
                {item.id === props.scope && <Check size={13} />}
              </button>
            ))}
          </div>
        )}
      </div>
      <span className="git-changes-review-branch" title={props.branch}><GitBranch size={12} />{props.branch || "HEAD"}</span>
      <div className="git-changes-review-tools">
        <button type="button" className="git-changes-review-icon" aria-expanded={menuOpen} aria-label={t("Changes options", "变更选项")} onClick={() => setMenuOpen((open) => !open)}>
          <Ellipsis size={14} />
        </button>
        {menuOpen && (
          <div className="git-changes-review-menu git-changes-review-menu-end" role="menu">
            <button type="button" role="menuitem" onClick={() => { updateDiffViewPreferences({ layout: "unified" }); setMenuOpen(false); }}>{t("Layout: Unified", "布局：统一")}</button>
            <button type="button" role="menuitem" onClick={() => { updateDiffViewPreferences({ layout: "side" }); setMenuOpen(false); }}>{t("Layout: Split", "布局：并排")}</button>
            <button type="button" role="menuitem" onClick={() => { updateDiffViewPreferences({ wrap: true }); setMenuOpen(false); }}>{t("Word Wrap", "自动换行")}</button>
            <button type="button" role="menuitem" onClick={() => { props.onFindingChange(true); setMenuOpen(false); }}>{t("Find in Changes", "在变更中查找")}</button>
            <button type="button" role="menuitem" onClick={() => { props.onExpandAll(); setMenuOpen(false); }}>{t("Expand All", "全部展开")}</button>
            <button type="button" role="menuitem" onClick={() => { props.onRefresh(); setMenuOpen(false); }}>{t("Refresh Changes", "刷新变更")}</button>
          </div>
        )}
        <Button className="git-changes-push" disabled={props.busy} onClick={() => void executeGitCommand("git.push", props.runOperation)}>{t("Push", "推送")}</Button>
      </div>
      {props.finding && (
        <label className="git-changes-review-find">
          <Search size={13} />
          <TextInput autoFocus value={props.query} onChange={(event) => props.onQueryChange(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") props.onFindingChange(false); }} placeholder={t("Find in changes", "在变更中查找")} aria-label={t("Find in changes", "在变更中查找")} />
        </label>
      )}
    </div>
  );
}
