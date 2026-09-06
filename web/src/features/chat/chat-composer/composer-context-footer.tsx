import { Undo2 } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { GoalControl } from "../../goals/goal-control";
import { useI18n } from "../../i18n/use-i18n";
import { createRunModeOptions } from "../../permission/run-mode-options";
import { TodoMarkdownView } from "../../todo/todo-markdown-view";
import { SystemUsage } from "../../usage/system-usage";
import type { ChatComposerProps } from "./composer-types";

/**
 * 将权限、目标和上下文统计放在输入框下方，保持正文编辑区集中。
 * @param props 会话操作和是否展示内置内核统计
 * @returns 输入区辅助控制栏
 */
export function ComposerContextFooter({ composer, showUsage }: { composer: ChatComposerProps; showUsage: boolean }) {
  const { t } = useI18n();
  return (
    <div className="composer-context-footer">
      <div className="composer-context-options">
        <div className="composer-mode"><Select value={composer.mode} options={createRunModeOptions(t)} ariaLabel={t("Run mode", "运行模式")} menuPreferredWidth={260} menuMinimumWidth={200} menuAlign="left" menuClassName="run-mode-menu" onChange={composer.onModeChange} /></div>
        <GoalControl sessionId={composer.sessionId} running={composer.running} draftValue={composer.value} onDraftChange={composer.onChange} onContinue={composer.onContinueGoal} />
        <TodoMarkdownView sessionId={composer.sessionId} compact />
      </div>
      <div className="composer-context-meta">
        {showUsage && <SystemUsage selection={composer.selection} mode={composer.mode} agentId={composer.agentSelection?.id} onCompact={composer.onCompact} compactDisabled={composer.running} />}
        {composer.undoAvailable && <Button variant="ghost" size="icon" className="composer-undo" onClick={composer.onUndo} disabled={composer.running} title={t("Step back to the previous turn", "退回上一轮")} aria-label={t("Undo last turn", "撤销最后一轮")}><Undo2 size={13} /></Button>}
      </div>
    </div>
  );
}
